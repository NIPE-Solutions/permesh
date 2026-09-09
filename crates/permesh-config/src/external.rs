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
/// with existing workspace approval contexts; selecting negotiated v1 requires fresh approval.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryProtocol {
    #[default]
    Legacy,
    NegotiatedV1,
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::network::present"
    )]
    pub sha256: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::pins::target_map"
    )]
    pub sha256_by_target: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "DiscoveryProtocol::is_legacy")]
    pub discovery_protocol: DiscoveryProtocol,
    #[serde(default)]
    pub configuration: BTreeMap<String, Value>,
    #[serde(default)]
    pub credentials: BTreeMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::network::present"
    )]
    pub network: Option<crate::NetworkConfig>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "crate::credential_resolvers::resolver_map"
    )]
    pub credential_resolvers: BTreeMap<String, crate::CredentialResolver>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::network::present"
    )]
    pub aws_profile: Option<crate::AwsProfile>,
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
    external.validate_pins()?;
    if let Some(network) = &external.network {
        if external.discovery_protocol != DiscoveryProtocol::NegotiatedV1 {
            return Err(invalid("external network requires negotiated_v1 discovery"));
        }
        network.validate()?;
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
        let reference = SecretRef::parse(reference).map_err(|_| {
            invalid("external credentials require explicit local or configured remote references")
        })?;
        if let SecretRef::Keychain { service, account } = reference
            && (service != provider.id || account != *name)
        {
            return Err(invalid(
                "external keychain reference must match the instance and credential name",
            ));
        }
    }
    crate::credential_resolvers::validate(provider)?;
    if let Some(profile) = &external.aws_profile {
        profile.validate(external)?;
    }
    Ok(())
}
