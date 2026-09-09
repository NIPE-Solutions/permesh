// SPDX-License-Identifier: MIT
//! Strict nonsecret host-owned resolver declarations; validation never resolves a token.
use crate::{NetworkConfig, Result, invalid};
use permesh_secrets::{RemoteBackend, SecretRef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum CredentialResolver {
    #[serde(rename = "1password_connect")]
    OnePasswordConnect {
        version: u32,
        origin: String,
        vault: String,
        item: String,
        field: String,
        bootstrap: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::network::present"
        )]
        network: Option<NetworkConfig>,
    },
    #[serde(rename = "vault_kv2")]
    VaultKv2 {
        version: u32,
        origin: String,
        mount: String,
        path: String,
        field: String,
        bootstrap: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::network::present"
        )]
        secret_version: Option<u64>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::network::present"
        )]
        network: Option<NetworkConfig>,
    },
    #[serde(rename = "openbao_kv2")]
    OpenBaoKv2 {
        version: u32,
        origin: String,
        mount: String,
        path: String,
        field: String,
        bootstrap: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::network::present"
        )]
        secret_version: Option<u64>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::network::present"
        )]
        network: Option<NetworkConfig>,
    },
}
impl std::fmt::Debug for CredentialResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CredentialResolver([REDACTED])")
    }
}
fn text(value: &str, limit: usize) -> bool {
    !value.is_empty()
        && value.len() <= limit
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
fn component(value: &str) -> bool {
    text(value, 256) && !matches!(value, "." | "..") && !value.contains(['%', '/', '\\', '?', '#'])
}
fn opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
impl CredentialResolver {
    pub fn backend(&self) -> RemoteBackend {
        match self {
            Self::OnePasswordConnect { .. } => RemoteBackend::OnePassword,
            Self::VaultKv2 { .. } => RemoteBackend::Vault,
            Self::OpenBaoKv2 { .. } => RemoteBackend::OpenBao,
        }
    }
    pub fn origin(&self) -> &str {
        match self {
            Self::OnePasswordConnect { origin, .. }
            | Self::VaultKv2 { origin, .. }
            | Self::OpenBaoKv2 { origin, .. } => origin,
        }
    }
    pub fn bootstrap(&self) -> &str {
        match self {
            Self::OnePasswordConnect { bootstrap, .. }
            | Self::VaultKv2 { bootstrap, .. }
            | Self::OpenBaoKv2 { bootstrap, .. } => bootstrap,
        }
    }
    pub fn field(&self) -> &str {
        match self {
            Self::OnePasswordConnect { field, .. }
            | Self::VaultKv2 { field, .. }
            | Self::OpenBaoKv2 { field, .. } => field,
        }
    }
    pub fn network(&self) -> Option<&NetworkConfig> {
        match self {
            Self::OnePasswordConnect { network, .. }
            | Self::VaultKv2 { network, .. }
            | Self::OpenBaoKv2 { network, .. } => network.as_ref(),
        }
    }
    pub fn validate(&self, instance: &str) -> Result<()> {
        let version = match self {
            Self::OnePasswordConnect { version, .. }
            | Self::VaultKv2 { version, .. }
            | Self::OpenBaoKv2 { version, .. } => *version,
        };
        let origin = self.origin();
        let url=url::Url::parse(origin).map_err(|_|invalid("resolver origin must be an explicit HTTPS origin without user information, path, query or fragment"))?;
        if version != 1
            || !text(origin, 2048)
            || origin.contains('\\')
            || !origin.starts_with("https://")
            || origin
                .strip_prefix("https://")
                .is_none_or(|authority| authority.trim_end_matches('/').contains('/'))
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid(
                "resolver requires version 1 and an explicit HTTPS origin without user information, path, query or fragment",
            ));
        }
        match SecretRef::parse(self.bootstrap()).map_err(|_|invalid("resolver bootstrap requires an explicit environment or same-instance keychain reference"))? {SecretRef::Env(_)=>(),SecretRef::Keychain{service,..} if service==instance=>(),_=>return Err(invalid("resolver bootstrap requires an explicit environment or same-instance keychain reference; recursive resolvers are unsupported"))}
        if !text(self.field(), 128) {
            return Err(invalid(
                "resolver field must be exact nonempty bounded text",
            ));
        }
        match self {
            Self::OnePasswordConnect {
                vault, item, field, ..
            } => {
                if vault.len() != 26
                    || item.len() != 26
                    || !opaque_id(vault)
                    || !opaque_id(item)
                    || !opaque_id(field)
                {
                    return Err(invalid(
                        "Connect requires exact 26-character vault/item IDs and a bounded field ID; labels are unsupported",
                    ));
                }
            }
            Self::VaultKv2 {
                mount,
                path,
                secret_version,
                ..
            }
            | Self::OpenBaoKv2 {
                mount,
                path,
                secret_version,
                ..
            } => {
                if !component(mount)
                    || path.len() > 1024
                    || !path.split('/').all(component)
                    || secret_version.is_some_and(|v| v == 0 || v > i64::MAX as u64)
                {
                    return Err(invalid(
                        "KV v2 requires an exact mount/path without traversal or encoded separators and an optional positive version",
                    ));
                }
            }
        }
        if let Some(network) = self.network() {
            network.validate()?;
        }
        Ok(())
    }
    pub fn target_url(&self) -> Result<url::Url> {
        let mut url =
            url::Url::parse(self.origin()).map_err(|_| invalid("invalid resolver origin"))?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| invalid("invalid resolver origin"))?;
            segments.clear().push("v1");
            match self {
                Self::OnePasswordConnect { vault, item, .. } => {
                    segments.push("vaults").push(vault).push("items").push(item);
                }
                Self::VaultKv2 { mount, path, .. } | Self::OpenBaoKv2 { mount, path, .. } => {
                    segments.push(mount).push("data");
                    for part in path.split('/') {
                        segments.push(part);
                    }
                }
            }
        }
        if let Self::VaultKv2 {
            secret_version: Some(version),
            ..
        }
        | Self::OpenBaoKv2 {
            secret_version: Some(version),
            ..
        } = self
        {
            url.query_pairs_mut()
                .append_pair("version", &version.to_string());
        }
        Ok(url)
    }
}
pub(crate) fn validate(provider: &crate::ProviderConfig) -> Result<()> {
    let Some(external) = &provider.external else {
        return Ok(());
    };
    if external.credential_resolvers.len() > 16 {
        return Err(invalid(
            "at most 16 explicit credential resolvers are supported",
        ));
    }
    let mut referenced = BTreeSet::new();
    for value in external.credentials.values() {
        if let SecretRef::Remote { backend, name } =
            SecretRef::parse(value).map_err(|_| invalid("invalid external credential reference"))?
        {
            let resolver = external.credential_resolvers.get(&name).ok_or_else(|| {
                invalid("remote credential reference has no resolver in this provider instance")
            })?;
            if resolver.backend() != backend {
                return Err(invalid(
                    "remote credential scheme does not match its resolver backend",
                ));
            }
            referenced.insert(name);
        }
    }
    for (name, resolver) in &external.credential_resolvers {
        if !referenced.contains(name) {
            return Err(invalid(
                "credential resolver declarations must be explicitly referenced by this provider's credential slots",
            ));
        }
        resolver.validate(&provider.id)?;
    }
    Ok(())
}
pub(crate) fn resolver_map<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<BTreeMap<String, CredentialResolver>, D::Error> {
    struct Unique;
    impl<'de> serde::de::Visitor<'de> for Unique {
        type Value = BTreeMap<String, CredentialResolver>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a unique explicit resolver map")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> std::result::Result<Self::Value, M::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, CredentialResolver>()? {
                if result.len() >= 16 || result.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom(
                        "duplicate or excessive resolver declarations",
                    ));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Unique)
}
