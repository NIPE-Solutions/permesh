// SPDX-License-Identifier: MIT
//! Declaration-only configuration for the host-local inventory data reader.
use crate::{ProviderConfig, ProviderKind, Result, invalid};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryConfig {
    pub path: String,
    pub sha256: String,
    #[serde(default = "default_age")]
    pub max_age_seconds: u64,
}
fn default_age() -> u64 {
    86_400
}
impl InventoryConfig {
    pub fn validate(&self) -> Result<()> {
        if self.path.is_empty()
            || self.path.len() > 1024
            || self.path.contains(['\\', ':'])
            || self.path.chars().any(char::is_control)
            || self
                .path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == ".." || part.trim() != part)
        {
            return Err(invalid(
                "inventory.path must be a workspace-relative file path without traversal, empty components, backslashes or drive prefixes",
            ));
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid(
                "inventory.sha256 must be exactly 64 lowercase hexadecimal characters",
            ));
        }
        if !(1..=604_800).contains(&self.max_age_seconds) {
            return Err(invalid(
                "inventory.max_age_seconds must be between 1 and 604800",
            ));
        }
        Ok(())
    }
}
pub(crate) fn validate(provider: &ProviderConfig) -> Result<()> {
    if provider.kind != ProviderKind::Inventory {
        return if provider.inventory.is_some() {
            Err(invalid("inventory configuration requires type inventory"))
        } else {
            Ok(())
        };
    }
    if provider.auth.is_some()
        || provider.external.is_some()
        || provider.customer_id.is_some()
        || !provider.organizations.is_empty()
    {
        return Err(invalid(
            "inventory providers accept inventory configuration only; authentication and executable settings are not supported",
        ));
    }
    provider
        .inventory
        .as_ref()
        .ok_or_else(|| invalid("inventory provider requires inventory configuration"))?
        .validate()
}
