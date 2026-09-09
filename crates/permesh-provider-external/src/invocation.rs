// SPDX-License-Identifier: MIT
//! Private, bounded operation inputs. Resolved credentials are never ordinary serde values.
use crate::ExternalError;
use permesh_provider_sdk::network::NetworkContext;
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

/// Keep the negotiated family distinct from legacy operation-specific drafts.
#[derive(Clone, Copy)]
pub(crate) enum Contract {
    Legacy(u32),
    Negotiated,
}
impl Serialize for Contract {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::Legacy(version) => map.serialize_entry("protocol", version)?,
            Self::Negotiated => map.serialize_entry(
                "protocol_version",
                &permesh_provider_protocol::negotiated::PROTOCOL_VERSION,
            )?,
        }
        map.end()
    }
}

pub struct Invocation {
    configuration: BTreeMap<String, Value>,
    credentials: BTreeMap<String, Secret>,
    network: Option<NetworkContext>,
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
            network: None,
        })
    }
    /// Attach validated, explicit network policy to negotiated operations only.
    pub fn with_network(mut self, network: NetworkContext) -> Result<Self, ExternalError> {
        network.validate().map_err(|_| ExternalError::Input)?;
        self.network = Some(network);
        Ok(self)
    }
    pub(crate) fn required_features(
        &self,
    ) -> &'static [permesh_provider_protocol::negotiated::Feature] {
        if self.network.is_some() {
            &[permesh_provider_protocol::negotiated::Feature::NetworkV1]
        } else {
            &[]
        }
    }
    pub(crate) fn validate_contract(&self, contract: Contract) -> Result<(), ExternalError> {
        if self.network.is_some() && !matches!(contract, Contract::Negotiated) {
            return Err(ExternalError::Input);
        }
        Ok(())
    }
    pub(crate) fn request(
        &self,
        method: &'static str,
        contract: Contract,
    ) -> Result<Zeroizing<Vec<u8>>, ExternalError> {
        if !matches!(contract, Contract::Legacy(2) | Contract::Negotiated)
            || !matches!(method, "check" | "discover")
        {
            return Err(ExternalError::Input);
        }
        self.validate_contract(contract)?;
        #[derive(Serialize)]
        struct Request<'a> {
            #[serde(flatten)]
            contract: Contract,
            id: &'static str,
            method: &'static str,
            configuration: &'a BTreeMap<String, Value>,
            credentials: BorrowedCredentials<'a>,
            #[serde(skip_serializing_if = "Option::is_none")]
            network: Option<&'a NetworkContext>,
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
                contract,
                id: method,
                method,
                configuration: &self.configuration,
                credentials: BorrowedCredentials(&self.credentials),
                network: self.network.as_ref(),
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
        let request = invocation.request("discover", Contract::Legacy(2)).unwrap();
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
            let bytes = invocation.request(method, Contract::Negotiated).unwrap();
            assert!(bytes.ends_with(b"\n"));
            let request: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(request["configuration"]["region"], "test");
            assert_eq!(request["protocol_version"], 1);
            assert!(request.get("protocol").is_none());
            assert_eq!(request["method"], method);
            assert_eq!(request["id"], method);
            assert_eq!(request["credentials"]["token"], "SENTINEL-private");
        }
    }
    #[test]
    fn network_context_is_validated_and_cannot_enter_legacy_requests() {
        assert!(
            invocation("SENTINEL")
                .with_network(NetworkContext::default())
                .is_err()
        );
        let context = NetworkContext {
            https_proxy: Some("http://127.0.0.1:3128".into()),
            no_proxy: vec![],
            ca_bundle_pem: None,
        };
        let invocation = invocation("SENTINEL")
            .with_network(context.clone())
            .unwrap();
        assert_eq!(
            invocation.required_features(),
            &[permesh_provider_protocol::negotiated::Feature::NetworkV1]
        );
        for method in ["check", "discover"] {
            assert!(invocation.request(method, Contract::Legacy(2)).is_err());
            let request: Value =
                serde_json::from_slice(&invocation.request(method, Contract::Negotiated).unwrap())
                    .unwrap();
            assert_eq!(request["network"], serde_json::to_value(&context).unwrap());
            assert_eq!(request["credentials"]["token"], "SENTINEL");
        }
        assert_eq!(format!("{invocation:?}"), "Invocation([REDACTED])");
    }
    #[test]
    fn configured_requests_reject_unsupported_versions_and_methods() {
        let invocation = invocation("SENTINEL-private");
        for version in [0, 1, 3, 4, 5, 6, u32::MAX] {
            assert!(matches!(
                invocation.request("discover", Contract::Legacy(version)),
                Err(ExternalError::Input)
            ));
        }
        for method in ["", "handshake", "cancel", "describe", "unknown"] {
            assert!(matches!(
                invocation.request(method, Contract::Negotiated),
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
        let request: Value =
            serde_json::from_slice(&invocation.request("check", Contract::Legacy(2)).unwrap())
                .unwrap();
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
