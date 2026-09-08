// SPDX-License-Identifier: MIT
//! Bounded metadata for explicitly selected official provider releases.
use crate::DistributionError;
use permesh_provider_sdk::Capability;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_CATALOG_BYTES: usize = 1024 * 1024;
pub const MAX_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;
pub const TARGETS: [&str; 5] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub releases: Vec<Release>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub provider: String,
    pub version: Version,
    pub target: String,
    pub capabilities: Vec<Capability>,
    pub protocols: Vec<u32>,
    pub archive_sha256: String,
    pub executable_sha256: String,
    pub archive_size: u64,
}

fn stable(version: &Version) -> bool {
    version.pre.is_empty() && version.build.is_empty()
}
fn provider_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id.as_bytes()[0].is_ascii_lowercase()
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn native_target() -> Option<&'static str> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") if cfg!(target_env = "gnu") => Some("x86_64-unknown-linux-gnu"),
        ("aarch64", "linux") if cfg!(target_env = "gnu") => Some("aarch64-unknown-linux-gnu"),
        ("x86_64", "macos") => Some("x86_64-apple-darwin"),
        ("aarch64", "macos") => Some("aarch64-apple-darwin"),
        ("x86_64", "windows") if cfg!(target_env = "msvc") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    }
}

pub fn validate_request(provider: &str, version: Option<&str>) -> Result<(), DistributionError> {
    if !provider_id(provider) {
        return Err(DistributionError::Input);
    }
    if let Some(text) = version {
        let version = Version::parse(text).map_err(|_| DistributionError::Input)?;
        if !stable(&version) || version.to_string() != text {
            return Err(DistributionError::Input);
        }
    }
    Ok(())
}

impl Release {
    pub fn validate(&self) -> Result<(), DistributionError> {
        if !provider_id(&self.provider)
            || !stable(&self.version)
            || !TARGETS.contains(&self.target.as_str())
            || !digest(&self.archive_sha256)
            || !digest(&self.executable_sha256)
            || !(1..=MAX_ARCHIVE_BYTES).contains(&self.archive_size)
            || !self.protocols.contains(&2)
            || self.protocols.iter().any(|v| !matches!(v, 2 | 3))
            || self
                .protocols
                .iter()
                .enumerate()
                .any(|(i, v)| self.protocols[..i].contains(v))
            || self
                .capabilities
                .iter()
                .enumerate()
                .any(|(i, v)| self.capabilities[..i].contains(v))
        {
            return Err(DistributionError::Catalog);
        }
        Ok(())
    }

    pub fn asset_url(&self) -> Result<String, DistributionError> {
        self.validate()?;
        Ok(format!(
            "https://github.com/NIPE-Solutions/permesh-providers/releases/download/{}-v{}/permesh-provider-{}-{}-{}.zip",
            self.provider, self.version, self.provider, self.version, self.target
        ))
    }
}

impl Catalog {
    pub fn validate(&self) -> Result<(), DistributionError> {
        if self.schema_version != 1 || self.releases.len() > 4096 {
            return Err(DistributionError::Catalog);
        }
        let mut seen = BTreeSet::new();
        for release in &self.releases {
            release.validate()?;
            if !seen.insert((&release.provider, &release.version, &release.target)) {
                return Err(DistributionError::Catalog);
            }
        }
        Ok(())
    }

    pub fn select(
        &self,
        provider: &str,
        target: &str,
        version: Option<&str>,
    ) -> Result<&Release, DistributionError> {
        self.validate()?;
        validate_request(provider, version)?;
        if !TARGETS.contains(&target) {
            return Err(DistributionError::Input);
        }
        let version = version
            .map(|text| {
                let value = Version::parse(text).map_err(|_| DistributionError::Input)?;
                if !stable(&value) || value.to_string() != text {
                    return Err(DistributionError::Input);
                }
                Ok(value)
            })
            .transpose()?;
        self.releases
            .iter()
            .filter(|r| {
                r.provider == provider
                    && r.target == target
                    && version.as_ref().is_none_or(|v| &r.version == v)
            })
            .max_by(|a, b| a.version.cmp(&b.version))
            .ok_or(DistributionError::Compatibility)
    }
}

pub fn parse(bytes: &[u8]) -> Result<Catalog, DistributionError> {
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(DistributionError::Catalog);
    }
    // Deserialize directly to structs: serde rejects duplicate and unknown fields,
    // unlike an intermediate JSON Value that would silently discard duplicates.
    let catalog: Catalog = serde_json::from_slice(bytes).map_err(|_| DistributionError::Catalog)?;
    catalog.validate()?;
    Ok(catalog)
}
