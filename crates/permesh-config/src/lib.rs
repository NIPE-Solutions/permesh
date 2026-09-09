// SPDX-License-Identifier: MIT
//! Strict, bounded workspace schema. YAML is data, never executable configuration.
mod data;
mod inventory;
pub use inventory::InventoryConfig;
mod external;
mod network;
mod pins;
pub use data::{load_setup_answers, parse_setup_value};
pub use external::{DiscoveryProtocol, ExternalConfig};
pub use network::{CaBundle, NetworkConfig};
use permesh_secrets::SecretRef;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
pub type Result<T> = std::result::Result<T, Error>;
pub const MAX_CONFIG_BYTES: usize = 1_048_576;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read workspace configuration")]
    Read,
    #[error("workspace configuration exceeds the 1 MiB limit")]
    TooLarge,
    #[error(
        "invalid workspace YAML; check schema, field spelling, duplicate keys, tags and parser limits"
    )]
    Parse,
    #[error(
        "invalid workspace YAML at line {line}, column {column}; check schema, duplicate keys and parser limits"
    )]
    ParseAt { line: u64, column: u64 },
    #[error("invalid configuration: {0}")]
    Validation(&'static str),
    #[error("no permesh.yaml found in this directory or its ancestors")]
    NotFound,
    #[error("cannot serialize workspace configuration")]
    Serialize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub organization: Organization,
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub identity: IdentityConfig,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Organization {
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ProviderKind,
    #[serde(default)]
    pub organizations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<ExternalConfig>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::network::present"
    )]
    pub inventory: Option<InventoryConfig>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Demo,
    /// Explicit host-local pinned identity inventory; never executable.
    Inventory,
    /// Legacy configuration accepted only for explicit migration; no bundled execution.
    Github,
    Google,
    External,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    pub token: String,
}
impl std::fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthConfig { token: [REFERENCE REDACTED] }")
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityConfig {
    #[serde(default)]
    pub sources: Vec<IdentitySource>,
    #[serde(default)]
    pub aliases: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentitySource {
    pub provider: String,
    #[serde(default)]
    pub authoritative: bool,
}
fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
fn invalid(message: &'static str) -> Error {
    Error::Validation(message)
}
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = data::read_file(path, MAX_CONFIG_BYTES)?;
        Self::from_bytes(&bytes)
    }
    /// Parse a captured workspace revision without reading it a second time.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_CONFIG_BYTES {
            return Err(Error::TooLarge);
        }
        let config: Self = data::parse(bytes, MAX_CONFIG_BYTES)?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(invalid("version must be 1"));
        }
        if !valid_text(&self.organization.name) {
            return Err(invalid(
                "organization.name must be nonempty text without controls",
            ));
        }
        let mut providers = BTreeMap::new();
        for provider in &self.providers {
            if !valid_id(&provider.id) {
                return Err(invalid(
                    "provider id must contain only ASCII letters, digits, hyphen or underscore",
                ));
            }
            if providers.insert(provider.id.as_str(), provider).is_some() {
                return Err(invalid("duplicate provider id"));
            }
            external::validate(provider)?;
            inventory::validate(provider)?;
            let mut orgs = BTreeSet::new();
            for org in &provider.organizations {
                if !valid_id(org) || org.contains('_') || org.starts_with('-') || org.ends_with('-')
                {
                    return Err(invalid("invalid provider organization name"));
                }
                if !orgs.insert(org.to_ascii_lowercase()) {
                    return Err(invalid("duplicate provider organization"));
                }
            }
            if provider.kind == ProviderKind::Demo && !provider.organizations.is_empty() {
                return Err(invalid("demo providers do not accept organizations"));
            }
            if provider.kind != ProviderKind::Google && provider.customer_id.is_some() {
                return Err(invalid("customer_id is only supported by Google providers"));
            }
            if provider.kind == ProviderKind::Google {
                if !provider.organizations.is_empty() {
                    return Err(invalid(
                        "Google providers use customer_id, not organizations",
                    ));
                }
                let customer = provider.customer_id.as_deref().unwrap_or_default();
                if !(2..=128).contains(&customer.len())
                    || !customer.starts_with('C')
                    || !customer.bytes().all(|c| c.is_ascii_alphanumeric())
                {
                    return Err(invalid(
                        "Google providers require an explicit customer_id starting with C followed by letters or digits",
                    ));
                }
            }
            match provider.kind {
                ProviderKind::Demo if provider.auth.is_some() => {
                    return Err(invalid("demo providers do not accept authentication"));
                }
                ProviderKind::Github | ProviderKind::Google => {
                    if provider.kind == ProviderKind::Github && provider.organizations.is_empty() {
                        return Err(invalid("GitHub providers require organizations"));
                    }
                    let auth = provider
                        .auth
                        .as_ref()
                        .ok_or_else(|| invalid("provider requires auth.token"))?;
                    let reference = SecretRef::parse(&auth.token)
                        .map_err(|_| invalid("auth.token must be an env or keychain reference"))?;
                    if let SecretRef::Keychain { service, account } = reference
                        && (service != provider.id || account != "token")
                    {
                        return Err(invalid("keychain reference must match its provider id"));
                    }
                }
                _ => {}
            }
        }
        let mut sources = BTreeSet::new();
        for source in &self.identity.sources {
            let provider = providers
                .get(source.provider.as_str())
                .ok_or_else(|| invalid("identity source references an unknown provider"))?;
            if !sources.insert(&source.provider) {
                return Err(invalid("duplicate identity source"));
            }
            if !matches!(
                provider.kind,
                ProviderKind::Demo
                    | ProviderKind::Google
                    | ProviderKind::External
                    | ProviderKind::Inventory
            ) {
                return Err(invalid(
                    "identity source provider lacks identity-source capability",
                ));
            }
        }
        let mut assigned = BTreeSet::new();
        for (alias, bindings) in &self.identity.aliases {
            if !valid_text(alias) || alias.trim() != alias || bindings.is_empty() {
                return Err(invalid("invalid or empty identity alias"));
            }
            for (provider, accounts) in bindings {
                if !providers.contains_key(provider.as_str()) || accounts.is_empty() {
                    return Err(invalid(
                        "alias must reference an existing provider and nonempty account list",
                    ));
                }
                for account in accounts {
                    if !valid_text(account)
                        || account.trim() != account
                        || account.contains(['/', '\\', '?', '#', '%'])
                        || account == "."
                        || account == ".."
                    {
                        return Err(invalid("invalid alias account id"));
                    }
                    if !assigned.insert((provider, account)) {
                        return Err(invalid("duplicate explicit account assignment"));
                    }
                }
            }
        }
        Ok(())
    }
}
pub fn to_yaml(config: &Config) -> Result<String> {
    config.validate()?;
    serde_saphyr::to_string(config).map_err(|_| Error::Serialize)
}
/// Return the nearest canonical permesh.yaml path; never merge ancestor workspaces.
pub fn find_workspace(start: &Path) -> Result<PathBuf> {
    let start = start.canonicalize().map_err(|_| Error::Read)?;
    let directory = if start.is_dir() {
        start.as_path()
    } else {
        start.parent().ok_or(Error::Read)?
    };
    for ancestor in directory.ancestors() {
        let path = ancestor.join("permesh.yaml");
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Read),
        }
    }
    Err(Error::NotFound)
}
