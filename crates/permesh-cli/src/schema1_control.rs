// SPDX-License-Identifier: MIT
//! CLI-owned schema 1 views of control-command data.
//!
//! Internal config, registry and catalog serde are not output contracts. Map each
//! field and capability explicitly, retaining order, nulls and independent schema
//! numbers. SDK SetupSpec/BrowserAuthSpec are already versioned public boundary
//! contracts and are deliberately embedded unchanged where exposed by the CLI.
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};

fn capabilities(values: &[permesh_provider_sdk::Capability]) -> Vec<&'static str> {
    use permesh_provider_sdk::Capability;
    values
        .iter()
        .map(|value| match value {
            Capability::Accounts => "accounts",
            Capability::Identities => "identities",
            Capability::Resources => "resources",
            Capability::Groups => "groups",
            Capability::Memberships => "memberships",
            Capability::Grants => "grants",
        })
        .collect()
}

#[derive(Serialize)]
pub(crate) struct Metadata<'a> {
    kind: &'a str,
    capabilities: Vec<&'static str>,
}
impl<'a> From<&'a permesh_provider_sdk::Metadata> for Metadata<'a> {
    fn from(value: &'a permesh_provider_sdk::Metadata) -> Self {
        Self {
            kind: &value.kind,
            capabilities: capabilities(&value.capabilities),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct IdentitySource<'a> {
    provider: &'a str,
    authoritative: bool,
}
impl<'a> From<&'a permesh_config::IdentitySource> for IdentitySource<'a> {
    fn from(value: &'a permesh_config::IdentitySource) -> Self {
        Self {
            provider: &value.provider,
            authoritative: value.authoritative,
        }
    }
}
pub(crate) fn identity_sources(
    values: &[permesh_config::IdentitySource],
) -> Vec<IdentitySource<'_>> {
    values.iter().map(IdentitySource::from).collect()
}

#[derive(Serialize)]
pub(crate) struct IdentityConfig<'a> {
    sources: Vec<IdentitySource<'a>>,
    aliases: &'a BTreeMap<String, BTreeMap<String, Vec<String>>>,
}
impl<'a> From<&'a permesh_config::IdentityConfig> for IdentityConfig<'a> {
    fn from(value: &'a permesh_config::IdentityConfig) -> Self {
        Self {
            sources: identity_sources(&value.sources),
            aliases: &value.aliases,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Inspection<'a> {
    sha256: &'a str,
    size: u64,
}
impl<'a> From<&'a permesh_provider_external::trust::Inspection> for Inspection<'a> {
    fn from(value: &'a permesh_provider_external::trust::Inspection) -> Self {
        Self {
            sha256: &value.sha256,
            size: value.size,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Registration<'a> {
    schema: u32,
    id: &'a str,
    sha256: &'a str,
    capabilities: Vec<&'static str>,
}
impl<'a> From<&'a permesh_provider_external::trust::Registration> for Registration<'a> {
    fn from(value: &'a permesh_provider_external::trust::Registration) -> Self {
        Self {
            schema: value.schema,
            id: &value.id,
            sha256: &value.sha256,
            capabilities: capabilities(&value.capabilities),
        }
    }
}
pub(crate) fn registrations(
    values: &[permesh_provider_external::trust::Registration],
) -> Vec<Registration<'_>> {
    values.iter().map(Registration::from).collect()
}

#[derive(Serialize)]
pub(crate) struct Approval<'a> {
    schema: u32,
    workspace: &'a Path,
    instance: &'a str,
    fingerprint: &'a str,
}
impl<'a> From<&'a permesh_provider_external::approvals::Approval> for Approval<'a> {
    fn from(value: &'a permesh_provider_external::approvals::Approval) -> Self {
        Self {
            schema: value.schema,
            workspace: &value.workspace,
            instance: &value.instance,
            fingerprint: &value.fingerprint,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Release<'a> {
    provider: &'a str,
    version: String,
    target: &'a str,
    capabilities: Vec<&'static str>,
    protocols: &'a [u32],
    #[serde(skip_serializing_if = "Option::is_none")]
    discovery_protocol: Option<&'static str>,
    archive_sha256: &'a str,
    executable_sha256: &'a str,
    archive_size: u64,
}
impl<'a> From<&'a permesh_provider_external::catalog::Release> for Release<'a> {
    fn from(value: &'a permesh_provider_external::catalog::Release) -> Self {
        Self {
            provider: &value.provider,
            version: value.version.to_string(),
            target: &value.target,
            capabilities: capabilities(&value.capabilities),
            protocols: &value.protocols,
            discovery_protocol: match value.discovery_protocol {
                permesh_provider_external::catalog::DiscoveryProtocol::Legacy => None,
                permesh_provider_external::catalog::DiscoveryProtocol::NegotiatedV1 => {
                    Some("negotiated_v1")
                }
            },
            archive_sha256: &value.archive_sha256,
            executable_sha256: &value.executable_sha256,
            archive_size: value.archive_size,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct InstalledPackage<'a> {
    release: Release<'a>,
    executable: &'a Path,
}
impl<'a> From<&'a permesh_provider_external::packages::InstalledPackage> for InstalledPackage<'a> {
    fn from(value: &'a permesh_provider_external::packages::InstalledPackage) -> Self {
        Self {
            release: Release::from(&value.release),
            executable: &value.executable,
        }
    }
}

#[cfg(test)]
mod release_tests {
    #[test]
    fn negotiated_contract_is_explicit_and_legacy_output_stays_unchanged()
    -> Result<(), Box<dyn std::error::Error>> {
        let legacy = serde_json::json!({"provider":"github","version":"0.1.0","target":"aarch64-apple-darwin","capabilities":["accounts"],"protocols":[2,3],"archive_sha256":"a".repeat(64),"executable_sha256":"b".repeat(64),"archive_size":123});
        let mut release: permesh_provider_external::catalog::Release =
            serde_json::from_value(legacy.clone())?;
        assert_eq!(
            serde_json::to_value(super::Release::from(&release))?,
            legacy
        );
        release.discovery_protocol =
            permesh_provider_external::catalog::DiscoveryProtocol::NegotiatedV1;
        release.protocols = vec![3];
        let value = serde_json::to_value(super::Release::from(&release))?;
        assert_eq!(value["discovery_protocol"], "negotiated_v1");
        assert_eq!(value["protocols"], serde_json::json!([3]));
        Ok(())
    }
}
