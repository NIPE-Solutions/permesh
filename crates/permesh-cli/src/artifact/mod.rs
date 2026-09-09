// SPDX-License-Identifier: MIT
pub mod diff;
mod mapping;
pub mod records;
mod storage;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub use storage::{check_destination, load, load_document, write, write_bytes, write_document};
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub format: String,
    pub format_version: u32,
    pub producer_version: String,
    pub started_at: String,
    pub completed_at: String,
    pub identity: IdentityContext,
    pub providers: Vec<ProviderCapture>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityContext {
    pub authorities: Vec<String>,
    pub bindings: Vec<Binding>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub identity: String,
    pub instance: String,
    pub account: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapture {
    pub instance: String,
    pub provider_type: String,
    pub provider_version: Option<String>,
    pub executable_sha256: Option<String>,
    pub capabilities: Vec<String>,
    pub context_sha256: String,
    pub configured_scope: std::collections::BTreeMap<String, Vec<String>>,
    pub started_at: String,
    pub completed_at: String,
    pub state: State,
    pub limitations: Vec<String>,
    pub data: Option<records::Dataset>,
    pub failure_code: Option<FailureCode>,
    pub source_observation: Option<SourceObservation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceObservation {
    FileInventory {
        declared_scope: String,
        exported_at: String,
        content_sha256: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Complete,
    Partial,
    Failed,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    ProviderCollectionFailed,
}
pub fn invalid() -> AppError {
    AppError::input(
        "Invalid or unsupported snapshot; check format version, bounds, scope and record references",
    )
}
pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn valid_digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn timestamp(s: &str) -> Result<time::OffsetDateTime, AppError> {
    let value = time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .map_err(|_| invalid())?;
    if value.offset() != time::UtcOffset::UTC {
        return Err(invalid());
    }
    Ok(value)
}
impl Artifact {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.format != "permesh_snapshot"
            || self.format_version != 1
            || self.producer_version.is_empty()
            || self.providers.is_empty()
            || self.providers.len() > 256
        {
            return Err(invalid());
        }
        let start = timestamp(&self.started_at)?;
        let end = timestamp(&self.completed_at)?;
        if start > end {
            return Err(invalid());
        }
        let mut instances = BTreeSet::new();
        for p in &self.providers {
            if p.instance.is_empty()
                || p.provider_type.is_empty()
                || !instances.insert(p.instance.as_str())
                || !valid_digest(&p.context_sha256)
                || p.executable_sha256
                    .as_ref()
                    .is_some_and(|s| !valid_digest(s))
            {
                return Err(invalid());
            }
            if timestamp(&p.started_at)? < start
                || timestamp(&p.completed_at)? > end
                || timestamp(&p.started_at)? > timestamp(&p.completed_at)?
            {
                return Err(invalid());
            }
            if let Some(SourceObservation::FileInventory {
                declared_scope,
                exported_at,
                content_sha256,
            }) = &p.source_observation
            {
                if p.provider_type != "inventory"
                    || declared_scope.is_empty()
                    || !valid_digest(content_sha256)
                {
                    return Err(invalid());
                }
                if timestamp(exported_at)? > timestamp(&p.completed_at)? {
                    return Err(invalid());
                }
            }
            if p.provider_type == "inventory"
                && p.state != State::Failed
                && p.source_observation.is_none()
            {
                return Err(invalid());
            }
            let caps: BTreeSet<_> = p.capabilities.iter().map(String::as_str).collect();
            if caps.len() != p.capabilities.len()
                || caps.iter().any(|c| {
                    !matches!(
                        *c,
                        "accounts"
                            | "identities"
                            | "resources"
                            | "groups"
                            | "memberships"
                            | "grants"
                    )
                })
            {
                return Err(invalid());
            }
            if (p.state == State::Failed) != p.failure_code.is_some() {
                return Err(invalid());
            }
            match (&p.data, p.state) {
                (None, State::Failed) => (),
                (Some(data), State::Complete | State::Partial) => {
                    if data.provider != p.instance || data.complete != (p.state == State::Complete)
                    {
                        return Err(invalid());
                    }
                    if data.limitations.iter().collect::<BTreeSet<_>>()
                        != p.limitations.iter().collect::<BTreeSet<_>>()
                    {
                        return Err(invalid());
                    }
                    for (name, count) in [
                        ("accounts", data.accounts.len()),
                        ("identities", data.identities.len()),
                        ("resources", data.resources.len()),
                        ("groups", data.groups.len()),
                        ("memberships", data.memberships.len()),
                        ("grants", data.grants.len()),
                    ] {
                        if count > 0 && !caps.contains(name) {
                            return Err(invalid());
                        }
                    }
                    let domain: permesh_core::Snapshot = data.into();
                    domain.validate().map_err(|_| invalid())?;
                }
                _ => return Err(invalid()),
            }
        }
        let authorities: BTreeSet<_> = self.identity.authorities.iter().collect();
        if authorities.len() != self.identity.authorities.len()
            || authorities.iter().any(|s| !instances.contains(s.as_str()))
        {
            return Err(invalid());
        }
        for p in &self.providers {
            if p.data.as_ref().is_some_and(|d| !d.identities.is_empty())
                && !authorities.contains(&p.instance)
            {
                return Err(invalid());
            }
        }
        let mut bindings = BTreeSet::new();
        for b in &self.identity.bindings {
            if b.identity.is_empty()
                || b.account.is_empty()
                || !instances.contains(b.instance.as_str())
                || !bindings.insert(b)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
    pub fn normalize(&mut self) {
        self.identity.authorities.sort();
        self.identity.bindings.sort();
        self.providers.sort_by(|a, b| a.instance.cmp(&b.instance));
        for p in &mut self.providers {
            p.capabilities.sort();
            p.limitations.sort();
            p.limitations.dedup();
            if let Some(data) = &mut p.data {
                let mut domain: permesh_core::Snapshot = (&*data).into();
                domain.sort();
                *data = (&domain).into();
            }
        }
    }
    pub fn complete(&self) -> bool {
        self.providers.iter().all(|p| p.state == State::Complete)
    }
}
