// SPDX-License-Identifier: MIT
//! Strict selected-field decoding with zeroizing owned secret strings and bounded input.
use super::{MAX_RESPONSE, MAX_SECRET, Resolved, error};
use permesh_config::CredentialResolver;
use permesh_secrets::Secret;
use serde::{
    Deserialize, Deserializer,
    de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use std::{collections::BTreeMap, fmt};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zeroize::Zeroizing;
#[derive(Deserialize)]
struct ConnectItem {
    id: String,
    vault: ConnectVault,
    fields: Vec<ConnectField>,
    version: u64,
}
#[derive(Deserialize)]
struct ConnectVault {
    id: String,
}
#[derive(Deserialize)]
struct ConnectField {
    id: String,
    #[serde(default)]
    value: Option<Zeroizing<String>>,
}
#[derive(Deserialize)]
struct KvEnvelope {
    data: KvData,
    #[serde(default)]
    lease_id: String,
    #[serde(default)]
    renewable: bool,
    #[serde(default)]
    lease_duration: u64,
}
#[derive(Deserialize)]
struct KvData {
    #[serde(deserialize_with = "fields")]
    data: BTreeMap<String, SecretValue>,
    metadata: KvMetadata,
}
#[derive(Deserialize)]
struct KvMetadata {
    version: u64,
    deletion_time: String,
    destroyed: bool,
}
struct SecretValue(Option<Zeroizing<String>>);
impl<'de> Deserialize<'de> for SecretValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Field;
        impl<'de> Visitor<'de> for Field {
            type Value = SecretValue;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a field value")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(SecretValue(Some(Zeroizing::new(value.to_owned()))))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(SecretValue(Some(Zeroizing::new(value))))
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self::Value, E> {
                Ok(SecretValue(None))
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self::Value, E> {
                Ok(SecretValue(None))
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self::Value, E> {
                Ok(SecretValue(None))
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self::Value, E> {
                Ok(SecretValue(None))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(SecretValue(None))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(SecretValue(None))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(SecretValue(None))
            }
        }
        deserializer.deserialize_any(Field)
    }
}
fn fields<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, SecretValue>, D::Error> {
    struct Fields;
    impl<'de> Visitor<'de> for Fields {
        type Value = BTreeMap<String, SecretValue>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an exact-field secret object")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, SecretValue>()? {
                if result.len() >= 1024 || result.insert(key, value).is_some() {
                    return Err(de::Error::custom("duplicate or excessive secret fields"));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Fields)
}
fn malformed() -> crate::error::AppError {
    error("Remote credential response is malformed or does not match the exact approved locator")
}
fn selected(
    mut value: Zeroizing<String>,
    version: u64,
    expires_at: Option<OffsetDateTime>,
) -> Result<Resolved, crate::error::AppError> {
    if value.is_empty() || value.len() > MAX_SECRET || version == 0 {
        return Err(error(
            "Remote credential field is empty, oversized or has invalid version metadata",
        ));
    }
    Ok(Resolved {
        secret: Secret::new(std::mem::take(&mut *value)),
        version,
        expires_at,
    })
}
pub(super) fn decode(
    config: &CredentialResolver,
    bytes: &[u8],
    now: OffsetDateTime,
) -> Result<Resolved, crate::error::AppError> {
    if bytes.len() > MAX_RESPONSE {
        return Err(error("Remote credential response exceeds the 64 KiB limit"));
    }
    match config {
        CredentialResolver::OnePasswordConnect {
            vault, item, field, ..
        } => {
            let mut value: ConnectItem = serde_json::from_slice(bytes).map_err(|_| malformed())?;
            if value.id != *item || value.vault.id != *vault || value.fields.len() > 1024 {
                return Err(malformed());
            }
            let mut matching = value
                .fields
                .iter_mut()
                .filter(|candidate| candidate.id == *field);
            let selected_value = matching
                .next()
                .and_then(|f| f.value.take())
                .ok_or_else(|| error("Remote credential field is missing or not a string"))?;
            if matching.next().is_some() {
                return Err(malformed());
            }
            selected(selected_value, value.version, None)
        }
        CredentialResolver::VaultKv2 {
            field,
            secret_version,
            ..
        }
        | CredentialResolver::OpenBaoKv2 {
            field,
            secret_version,
            ..
        } => {
            let mut value: KvEnvelope = serde_json::from_slice(bytes).map_err(|_| malformed())?;
            if !value.lease_id.is_empty() || value.renewable || value.lease_duration != 0 {
                return Err(error(
                    "Dynamic secret leases are unsupported by the KV v2 resolver",
                ));
            }
            let meta = value.data.metadata;
            if meta.destroyed || secret_version.is_some_and(|expected| expected != meta.version) {
                return Err(error(
                    "Remote credential version is destroyed or does not match the requested version",
                ));
            }
            let expires_at = if meta.deletion_time.is_empty() {
                None
            } else {
                let expiry = OffsetDateTime::parse(&meta.deletion_time, &Rfc3339)
                    .map_err(|_| malformed())?;
                if expiry <= now {
                    return Err(error(
                        "Remote credential version has been deleted or expired",
                    ));
                }
                Some(expiry)
            };
            let selected_value = value
                .data
                .data
                .remove(field)
                .and_then(|value| value.0)
                .ok_or_else(|| error("Remote credential field is missing or not a string"))?;
            selected(selected_value, meta.version, expires_at)
        }
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;
    fn config(kind: &str) -> CredentialResolver {
        serde_json::from_value(json!({"version":1,"type":kind,"origin":"https://secrets.example.test","mount":"secret","path":"providers/api","field":"token","bootstrap":"env://BOOTSTRAP","secret_version":2})).unwrap()
    }
    fn body() -> serde_json::Value {
        json!({"data":{"data":{"token":"SELECTED_SENTINEL","other":"UNRELATED_SENTINEL","nested":{"token":"NOT_SELECTED"}},"metadata":{"version":2,"deletion_time":"","destroyed":false}}})
    }
    #[test]
    fn both_kv_backends_select_exact_top_level_string_and_respect_versions() {
        for kind in ["vault_kv2", "openbao_kv2"] {
            let config = config(kind);
            let got = decode(
                &config,
                body().to_string().as_bytes(),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
            assert_eq!(got.secret.expose(), "SELECTED_SENTINEL");
            assert_eq!(got.version, 2);
            assert!(got.expires_at.is_none());
            for bad in [json!(42), json!({"token":"NESTED"}), json!(null), json!("")] {
                let mut value = body();
                value["data"]["data"]["token"] = bad;
                assert!(
                    decode(
                        &config,
                        value.to_string().as_bytes(),
                        OffsetDateTime::now_utc()
                    )
                    .is_err()
                );
            }
            for field in ["destroyed", "version", "deletion_time"] {
                let mut value = body();
                value["data"]["metadata"][field] = match field {
                    "destroyed" => json!(true),
                    "version" => json!(1),
                    _ => json!("2020-01-01T00:00:00Z"),
                };
                let error = decode(
                    &config,
                    value.to_string().as_bytes(),
                    OffsetDateTime::now_utc(),
                )
                .err()
                .unwrap();
                assert!(!error.message.contains("SENTINEL"));
            }
        }
    }
    #[test]
    fn connect_checks_item_identity_and_unique_exact_field() {
        let config:CredentialResolver=serde_json::from_value(json!({"version":1,"type":"1password_connect","origin":"https://connect.example.test","vault":"abcdefghijklmnopqrstuvwxyz","item":"zyxwvutsrqponmlkjihgfedcba","field":"password","bootstrap":"env://BOOTSTRAP"})).unwrap();
        let mut body = json!({"id":"zyxwvutsrqponmlkjihgfedcba","vault":{"id":"abcdefghijklmnopqrstuvwxyz"},"version":3,"fields":[{"id":"username","value":"UNRELATED"},{"id":"password","value":"SELECTED"}]});
        assert_eq!(
            decode(
                &config,
                body.to_string().as_bytes(),
                OffsetDateTime::now_utc()
            )
            .unwrap()
            .secret
            .expose(),
            "SELECTED"
        );
        body["fields"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"password","value":"AMBIGUOUS"}));
        assert!(
            decode(
                &config,
                body.to_string().as_bytes(),
                OffsetDateTime::now_utc()
            )
            .is_err()
        );
        body["id"] = json!("another");
        assert!(
            decode(
                &config,
                body.to_string().as_bytes(),
                OffsetDateTime::now_utc()
            )
            .is_err()
        );
    }
    #[test]
    fn backend_metadata_duplicates_missing_and_expiring_values_fail_closed() {
        let now = OffsetDateTime::from_unix_timestamp(1_000_000).unwrap();
        for kind in ["vault_kv2", "openbao_kv2"] {
            let config = config(kind);
            let mut future = body();
            future["data"]["metadata"]["deletion_time"] = json!(
                (now + time::Duration::seconds(60))
                    .format(&Rfc3339)
                    .unwrap()
            );
            assert_eq!(
                decode(&config, future.to_string().as_bytes(), now)
                    .unwrap()
                    .expires_at,
                Some(now + time::Duration::seconds(60))
            );
            for bad in [
                json!({"data":null}),
                json!({"data":{"data":null,"metadata":{"version":2,"destroyed":false,"deletion_time":""}}}),
            ] {
                assert!(decode(&config, bad.to_string().as_bytes(), now).is_err());
            }
            for (key, value) in [
                ("lease_id", json!("LEASE_SENTINEL")),
                ("renewable", json!(true)),
                ("lease_duration", json!(10)),
            ] {
                let mut bad = body();
                bad[key] = value;
                let error = decode(&config, bad.to_string().as_bytes(), now)
                    .err()
                    .unwrap();
                assert!(!error.message.contains("SENTINEL"));
            }
            let mut missing = body();
            missing["data"]["data"] = json!({"different":"SENTINEL"});
            assert!(decode(&config, missing.to_string().as_bytes(), now).is_err());
            let mut huge = body();
            huge["data"]["data"]["token"] = json!("x".repeat(MAX_SECRET + 1));
            assert!(decode(&config, huge.to_string().as_bytes(), now).is_err());
            let duplicate = body()
                .to_string()
                .replace("\"version\":2", "\"version\":2,\"version\":2");
            assert!(decode(&config, duplicate.as_bytes(), now).is_err());
            let duplicate = body().to_string().replace(
                "\"token\":\"SELECTED_SENTINEL\"",
                "\"token\":\"SELECTED_SENTINEL\",\"token\":\"OTHER\"",
            );
            assert!(decode(&config, duplicate.as_bytes(), now).is_err());
        }
    }
}
