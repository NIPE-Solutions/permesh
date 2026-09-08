// SPDX-License-Identifier: MIT
use crate::{ProviderConfig, ProviderKind, Result, invalid};
use permesh_secrets::SecretRef;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

const MAX_PARAMETERS_BYTES: usize = 65_536;
const MAX_PARAMETER_DEPTH: usize = 16;
const MAX_CREDENTIALS: usize = 16;

/// Explicit discovery/health wire family. Default serialization remains byte-compatible
/// with existing workspace approval contexts; selecting v5 requires fresh approval.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryProtocol {
    #[default]
    Legacy,
    NegotiatedV5,
}
impl DiscoveryProtocol {
    fn is_legacy(&self) -> bool {
        matches!(self, Self::Legacy)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalConfig {
    pub provider: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "DiscoveryProtocol::is_legacy")]
    pub discovery_protocol: DiscoveryProtocol,
    #[serde(default)]
    pub configuration: BTreeMap<String, Value>,
    #[serde(default)]
    pub credentials: BTreeMap<String, String>,
}

fn identifier(value: &str) -> bool {
    value.len() <= 64
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

fn bounded_depth(value: &Value, depth: usize) -> bool {
    depth <= MAX_PARAMETER_DEPTH
        && match value {
            Value::Array(values) => values.iter().all(|v| bounded_depth(v, depth + 1)),
            Value::Object(values) => values.values().all(|v| bounded_depth(v, depth + 1)),
            _ => true,
        }
}

pub(crate) fn validate(provider: &ProviderConfig) -> Result<()> {
    if provider.kind != ProviderKind::External {
        return if provider.external.is_some() {
            Err(invalid("external configuration requires type external"))
        } else {
            Ok(())
        };
    }
    let external = provider
        .external
        .as_ref()
        .ok_or_else(|| invalid("external provider requires external configuration"))?;
    if !identifier(&provider.id) || !identifier(&external.provider) {
        return Err(invalid(
            "external provider and instance ids require an ASCII letter followed by letters, digits, hyphen or underscore (maximum 64 bytes)",
        ));
    }
    if !provider.organizations.is_empty()
        || provider.customer_id.is_some()
        || provider.auth.is_some()
    {
        return Err(invalid(
            "external providers use external.configuration and external.credentials only",
        ));
    }
    if external.sha256.len() != 64
        || !external
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid(
            "external.sha256 must contain 64 lowercase hexadecimal characters",
        ));
    }
    let bytes = serde_json::to_vec(&external.configuration)
        .map_err(|_| invalid("external configuration must be JSON compatible"))?;
    if bytes.len() > MAX_PARAMETERS_BYTES
        || !external.configuration.values().all(|v| bounded_depth(v, 1))
    {
        return Err(invalid(
            "external configuration exceeds its 64 KiB or 16-level limit",
        ));
    }
    if external.credentials.len() > MAX_CREDENTIALS {
        return Err(invalid("external credentials exceed the 16-slot limit"));
    }
    for (name, reference) in &external.credentials {
        if !identifier(name) {
            return Err(invalid("invalid external credential name"));
        }
        let reference = SecretRef::parse(reference)
            .map_err(|_| invalid("external credentials require env or keychain references"))?;
        if let SecretRef::Keychain { service, account } = reference
            && (service != provider.id || account != *name)
        {
            return Err(invalid(
                "external keychain reference must match the instance and credential name",
            ));
        }
    }
    Ok(())
}
