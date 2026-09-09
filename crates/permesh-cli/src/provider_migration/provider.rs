// SPDX-License-Identifier: MIT
use crate::error::AppError;
use permesh_config::{ExternalConfig, ProviderConfig, ProviderKind};
use permesh_provider_sdk::Capability;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) enum Legacy {
    Github,
    Google,
}
impl Legacy {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Google => "google",
        }
    }
    pub(super) fn capabilities(self) -> &'static [Capability] {
        match self {
            Self::Github => &[
                Capability::Accounts,
                Capability::Resources,
                Capability::Groups,
                Capability::Memberships,
                Capability::Grants,
            ],
            Self::Google => &[Capability::Accounts, Capability::Identities],
        }
    }
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::Github => {
                "Migrated GitHub instance; ID, organizations, aliases and token reference were preserved."
            }
            Self::Google => {
                "Migrated Google instance; ID, customer, identity authority, aliases and token reference were preserved. Authentication remains access-token based."
            }
        }
    }
}

pub(super) fn convert(
    provider: &mut ProviderConfig,
    sha256: &str,
    discovery_protocol: permesh_config::DiscoveryProtocol,
) -> Result<Legacy, AppError> {
    let kind = match provider.kind {
        ProviderKind::Github => Legacy::Github,
        ProviderKind::Google => Legacy::Google,
        _ => {
            return Err(AppError::input(
                "Migration requires an existing type: github or type: google instance; external instances are already migrated",
            ));
        }
    };
    if provider.id.len() > 64
        || !provider
            .id
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
    {
        return Err(AppError::input(
            "External instance IDs must start with an ASCII letter and contain at most 64 bytes. Rename this legacy ID and its aliases/keychain references explicitly before migration",
        ));
    }
    let configuration = match kind {
        Legacy::Github => {
            if provider.organizations.len() > 100
                || provider.organizations.iter().any(|org| org.len() > 100)
            {
                return Err(AppError::input(
                    "External GitHub supports at most 100 organizations with names at most 100 bytes; edit the legacy configuration explicitly before migration",
                ));
            }
            BTreeMap::from([(
                "organizations".into(),
                serde_json::json!(std::mem::take(&mut provider.organizations)),
            )])
        }
        Legacy::Google => {
            let customer = provider
                .customer_id
                .take()
                .ok_or_else(|| AppError::input("Google customer ID is missing"))?;
            BTreeMap::from([
                ("customer_id".into(), serde_json::json!(customer)),
                ("auth_mode".into(), serde_json::json!("access_token")),
            ])
        }
    };
    let token = provider
        .auth
        .take()
        .ok_or_else(|| AppError::input("Provider token reference is missing"))?
        .token;
    provider.kind = ProviderKind::External;
    provider.external = Some(ExternalConfig {
        network: None,
        discovery_protocol,
        provider: kind.name().into(),
        sha256: Some(sha256.into()),
        sha256_by_target: None,
        configuration,
        credentials: BTreeMap::from([("token".into(), token)]),
    });
    Ok(kind)
}
