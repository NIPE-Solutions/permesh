// SPDX-License-Identifier: MIT
//! Private, bounded operation inputs. Resolved credentials are never ordinary serde values.
use crate::ExternalError;
use permesh_secrets::Secret;
use serde::{Serialize, Serializer, ser::SerializeMap};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fmt,
    io::{self, Write},
};
use zeroize::Zeroizing;

const MAX_CONFIGURATION: usize = 64 * 1024;
const MAX_CREDENTIALS: usize = 16;
const MAX_CREDENTIAL: usize = 16 * 1024;
const MAX_CREDENTIAL_BYTES: usize = 64 * 1024;

pub struct Invocation {
    configuration: BTreeMap<String, Value>,
    credentials: BTreeMap<String, Secret>,
}
impl fmt::Debug for Invocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invocation([REDACTED])")
    }
}
impl Invocation {
    pub fn new(
        configuration: BTreeMap<String, Value>,
        credentials: BTreeMap<String, Secret>,
    ) -> Result<Self, ExternalError> {
        if credentials.len() > MAX_CREDENTIALS || credentials.keys().any(|key| !valid_name(key)) {
            return Err(ExternalError::Input);
        }
        let mut total = 0usize;
        for secret in credentials.values() {
            let size = secret.expose().len();
            if size == 0 || size > MAX_CREDENTIAL {
                return Err(ExternalError::Input);
            }
            total += size;
        }
        if total > MAX_CREDENTIAL_BYTES
            || configuration.values().any(|value| !within_depth(value, 1))
        {
            return Err(ExternalError::Input);
        }
        let mut counter = Limited {
            output: io::sink(),
            remaining: MAX_CONFIGURATION,
        };
        serde_json::to_writer(&mut counter, &configuration).map_err(|_| ExternalError::Input)?;
        Ok(Self {
            configuration,
            credentials,
        })
    }
    #[cfg(test)]
    pub(crate) fn request(
        &self,
        method: &'static str,
    ) -> Result<Zeroizing<Vec<u8>>, ExternalError> {
        self.request_versioned(method, 2)
    }
    pub(crate) fn request_versioned(
        &self,
        method: &'static str,
        protocol: u32,
    ) -> Result<Zeroizing<Vec<u8>>, ExternalError> {
        if !matches!(
            protocol,
            2 | permesh_provider_protocol::negotiated::PROTOCOL_VERSION
        ) || !matches!(method, "check" | "discover")
        {
            return Err(ExternalError::Input);
        }
        #[derive(Serialize)]
        struct Request<'a> {
            protocol: u32,
            id: &'static str,
            method: &'static str,
            configuration: &'a BTreeMap<String, Value>,
            credentials: BorrowedCredentials<'a>,
        }
        // Reserve the entire bound before any secret is written so reallocations
        // cannot leave old plaintext allocations behind. Drop zeroizes the buffer.
        let mut bytes = Zeroizing::new(Vec::with_capacity(
            permesh_provider_protocol::MAX_FRAME_BYTES,
        ));
        let mut writer = Limited {
            output: &mut *bytes,
            remaining: permesh_provider_protocol::MAX_FRAME_BYTES,
        };
        serde_json::to_writer(
            &mut writer,
            &Request {
                protocol,
                id: method,
                method,
                configuration: &self.configuration,
                credentials: BorrowedCredentials(&self.credentials),
            },
        )
        .map_err(|_| ExternalError::Input)?;
        writer.write_all(b"\n").map_err(|_| ExternalError::Input)?;
        Ok(bytes)
    }
    pub(crate) fn rejects_reflection(&self, frame: &[u8]) -> Result<bool, ExternalError> {
        if self.credentials.is_empty() {
            return Ok(false);
        }
        let value: Value = serde_json::from_slice(frame).map_err(|_| ExternalError::Protocol)?;
        Ok(reflects(&value, &self.credentials))
    }
}
struct BorrowedCredentials<'a>(&'a BTreeMap<String, Secret>);
impl Serialize for BorrowedCredentials<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, secret) in self.0 {
            map.serialize_entry(name, secret.expose())?;
        }
        map.end()
    }
}
struct Limited<W> {
    output: W,
    remaining: usize,
}
impl<W: Write> Write for Limited<W> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if input.len() > self.remaining {
            return Err(io::Error::other("input exceeds limit"));
        }
        let count = self.output.write(input)?;
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}
fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}
fn within_depth(value: &Value, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    match value {
        Value::Array(items) => items.iter().all(|value| within_depth(value, depth + 1)),
        Value::Object(items) => items.values().all(|value| within_depth(value, depth + 1)),
        _ => true,
    }
}
fn reflects(value: &Value, secrets: &BTreeMap<String, Secret>) -> bool {
    let contains = |value: &str| {
        secrets
            .values()
            .any(|secret| value.contains(secret.expose()))
    };
    match value {
        Value::String(value) => contains(value),
        Value::Array(values) => values.iter().any(|value| reflects(value, secrets)),
        Value::Object(values) => values
            .iter()
            .any(|(key, value)| contains(key) || reflects(value, secrets)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    fn invocation(secret: &str) -> Invocation {
        Invocation::new(
            BTreeMap::new(),
            BTreeMap::from([("token".into(), Secret::new(secret.into()))]),
        )
        .unwrap()
    }
    #[test]
    fn accepts_named_bounded_credentials_without_debug_disclosure() {
        let invocation = invocation("SENTINEL-private");
        assert_eq!(format!("{invocation:?}"), "Invocation([REDACTED])");
        let request = invocation.request("discover").unwrap();
        assert_eq!(request.as_slice(), b"{\"protocol\":2,\"id\":\"discover\",\"method\":\"discover\",\"configuration\":{},\"credentials\":{\"token\":\"SENTINEL-private\"}}\n");
        let request: Value = serde_json::from_slice(&request).unwrap();
        assert_eq!(request["credentials"]["token"], "SENTINEL-private");
        assert_eq!(request["protocol"], 2);
    }
    #[test]
    fn negotiated_request_preserves_configuration_and_private_credentials() {
        let invocation = Invocation::new(
            BTreeMap::from([("region".into(), serde_json::json!("test"))]),
            BTreeMap::from([("token".into(), Secret::new("SENTINEL-private".into()))]),
        )
        .unwrap();
        for method in ["check", "discover"] {
            let bytes = invocation.request_versioned(method, 5).unwrap();
            assert!(bytes.ends_with(b"\n"));
            let request: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(request["configuration"]["region"], "test");
            assert_eq!(request["protocol"], 5);
            assert_eq!(request["method"], method);
            assert_eq!(request["id"], method);
            assert_eq!(request["credentials"]["token"], "SENTINEL-private");
        }
    }
    #[test]
    fn configured_requests_reject_unsupported_versions_and_methods() {
        let invocation = invocation("SENTINEL-private");
        for version in [0, 1, 3, 4, 6, u32::MAX] {
            assert!(matches!(
                invocation.request_versioned("discover", version),
                Err(ExternalError::Input)
            ));
        }
        for method in ["", "handshake", "cancel", "describe", "unknown"] {
            assert!(matches!(
                invocation.request_versioned(method, 5),
                Err(ExternalError::Input)
            ));
        }
    }
    #[test]
    fn input_budgets_and_names_fail_closed() {
        for secret in ["".to_owned(), "x".repeat(MAX_CREDENTIAL + 1)] {
            assert!(
                Invocation::new(
                    BTreeMap::new(),
                    BTreeMap::from([("token".into(), Secret::new(secret))])
                )
                .is_err()
            );
        }
        assert!(
            Invocation::new(
                BTreeMap::new(),
                (0..17)
                    .map(|i| (format!("slot{i}"), Secret::new("x".into())))
                    .collect()
            )
            .is_err()
        );
        assert!(
            Invocation::new(
                BTreeMap::new(),
                (0..5)
                    .map(|i| (format!("slot{i}"), Secret::new("x".repeat(MAX_CREDENTIAL))))
                    .collect()
            )
            .is_err()
        );
        for key in ["", "9bad", "line\nbreak"] {
            assert!(
                Invocation::new(
                    BTreeMap::new(),
                    BTreeMap::from([(key.into(), Secret::new("x".into()))])
                )
                .is_err()
            );
        }
        assert!(
            Invocation::new(
                BTreeMap::from([(
                    "setting".into(),
                    Value::String("x".repeat(MAX_CONFIGURATION))
                )]),
                BTreeMap::new()
            )
            .is_err()
        );
        let mut value = Value::Null;
        for _ in 0..17 {
            value = Value::Array(vec![value]);
        }
        assert!(
            Invocation::new(BTreeMap::from([("deep".into(), value)]), BTreeMap::new()).is_err()
        );
    }
    #[test]
    fn configuration_keys_are_provider_specific_json() {
        let invocation = Invocation::new(
            BTreeMap::from([(
                "endpoint.url[primary]".into(),
                Value::String("https://example.invalid".into()),
            )]),
            BTreeMap::new(),
        )
        .unwrap();
        let request: Value = serde_json::from_slice(&invocation.request("check").unwrap()).unwrap();
        assert_eq!(
            request["configuration"]["endpoint.url[primary]"],
            "https://example.invalid"
        );
    }
    #[test]
    fn reflection_scan_decodes_escaped_values_and_keys() {
        let invocation = invocation("SENTINEL");
        for frame in [
            br#"{"name":"prefix SENTINEL suffix"}"#.as_slice(),
            br#"{"name":"\u0053ENTINEL"}"#,
            br#"{"\u0053ENTINEL":null}"#,
        ] {
            assert!(invocation.rejects_reflection(frame).unwrap());
        }
        assert!(
            !invocation
                .rejects_reflection(br#"{"name":"safe"}"#)
                .unwrap()
        );
    }
}
