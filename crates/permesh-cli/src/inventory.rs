// SPDX-License-Identifier: MIT
//! Pinned host-local JSON inventory. This reader never executes code or resolves credentials.
use crate::error::AppError;
use permesh_config::{ProviderConfig, ProviderKind};
use permesh_core::{Affiliation, Identity, IdentityKind, IdentityStatus, Snapshot};
use permesh_provider_sdk::{Capability, Metadata};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs::File, io::Read, path::Path};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
pub(crate) const MAX_INVENTORY_BYTES: usize = 8 * 1024 * 1024;
const MAX_IDENTITIES: usize = 100_000;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    scope: String,
    exported_at: String,
    complete: bool,
    identities: Vec<Record>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    id: String,
    kind: RecordKind,
    affiliation: RecordAffiliation,
    lifecycle: RecordLifecycle,
}
// These enums belong to inventory format v1, independent of evolving domain enums.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RecordKind {
    Human,
    Service,
    Bot,
    Unknown,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RecordAffiliation {
    Internal,
    External,
    Unknown,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RecordLifecycle {
    Active,
    Inactive,
    Suspended,
    Unknown,
}
pub(crate) struct Observation {
    pub snapshot: Snapshot,
}
pub(crate) fn metadata() -> Metadata {
    Metadata {
        kind: "inventory".into(),
        capabilities: vec![Capability::Identities],
    }
}
fn error(message: &'static str) -> AppError {
    AppError::new(3, message)
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn text(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= 256
        && !value.chars().any(char::is_control)
}
fn decode(
    id: &str,
    bytes: &[u8],
    max_age: u64,
    now: OffsetDateTime,
) -> Result<Observation, AppError> {
    if bytes.len() > MAX_INVENTORY_BYTES {
        return Err(error("Inventory exceeds the 8 MiB limit"));
    }
    let document: Document = serde_json::from_slice(bytes).map_err(|_| {
        error("Inventory must be strict version-1 JSON with unique fields and supported values")
    })?;
    if document.version != 1 || !text(&document.scope) || document.identities.len() > MAX_IDENTITIES
    {
        return Err(error(
            "Inventory requires version 1, a bounded declared scope and at most 100000 identities",
        ));
    }
    let exported = OffsetDateTime::parse(&document.exported_at, &Rfc3339)
        .map_err(|_| error("Inventory exported_at must be a UTC RFC3339 timestamp"))?;
    if !exported.offset().is_utc() || exported > now {
        return Err(error(
            "Inventory exported_at must be UTC and must not be in the future",
        ));
    }
    let stale = (now - exported)
        > time::Duration::seconds(
            i64::try_from(max_age).map_err(|_| error("Invalid inventory freshness bound"))?,
        );
    let mut snapshot = Snapshot::new(id);
    snapshot.complete = document.complete && !stale;
    if !document.complete {
        snapshot.limitations.push("Inventory export is explicitly incomplete; omitted identities cannot establish offboarding".into());
    }
    if stale {
        snapshot.limitations.push("Inventory export exceeds the configured freshness bound; authority assessment is incomplete".into());
    }
    let mut ids = BTreeSet::new();
    for record in document.identities {
        if !text(&record.id) || !ids.insert(record.id.clone()) {
            return Err(error(
                "Inventory canonical IDs must be unique nonempty bounded text without controls",
            ));
        }
        snapshot.identities.push(Identity {
            id: record.id,
            kind: match record.kind {
                RecordKind::Human => IdentityKind::Human,
                RecordKind::Service => IdentityKind::Service,
                RecordKind::Bot => IdentityKind::Bot,
                RecordKind::Unknown => IdentityKind::Unknown,
            },
            affiliation: match record.affiliation {
                RecordAffiliation::Internal => Affiliation::Internal,
                RecordAffiliation::External => Affiliation::External,
                RecordAffiliation::Unknown => Affiliation::Unknown,
            },
            status: match record.lifecycle {
                RecordLifecycle::Active => IdentityStatus::Active,
                RecordLifecycle::Inactive => IdentityStatus::Inactive,
                RecordLifecycle::Suspended => IdentityStatus::Suspended,
                RecordLifecycle::Unknown => IdentityStatus::Unknown,
            },
            verified_emails: vec![],
        });
    }
    snapshot.sort();
    snapshot
        .validate()
        .map_err(|_| error("Inventory observations violate the identity contract"))?;
    Ok(Observation { snapshot })
}
pub(crate) fn observe(
    provider: &ProviderConfig,
    workspace: &Path,
) -> Result<Observation, AppError> {
    observe_at(provider, workspace, OffsetDateTime::now_utc())
}
fn observe_at(
    provider: &ProviderConfig,
    workspace: &Path,
    now: OffsetDateTime,
) -> Result<Observation, AppError> {
    if provider.kind != ProviderKind::Inventory {
        return Err(error("Inventory reader requires type inventory"));
    }
    let settings = provider
        .inventory
        .as_ref()
        .ok_or_else(|| error("Inventory configuration is missing"))?;
    settings
        .validate()
        .map_err(|_| error("Inventory configuration is invalid"))?;
    let workspace = workspace
        .canonicalize()
        .map_err(|_| error("Cannot locate inventory workspace"))?;
    let root = workspace
        .parent()
        .ok_or_else(|| error("Cannot locate inventory workspace"))?;
    let mut path = root.to_path_buf();
    // Reject every symlink component, including links that remain inside the workspace.
    // This portable precheck is not protection against a hostile same-user filesystem race.
    for component in settings.path.split('/') {
        path.push(component);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| error("Cannot read configured inventory file"))?;
        if metadata.file_type().is_symlink() {
            return Err(error(
                "Inventory file and parent directories must not be symlinks",
            ));
        }
    }
    if !std::fs::metadata(&path)
        .map_err(|_| error("Cannot read configured inventory file"))?
        .is_file()
    {
        return Err(error("Inventory source must be a regular file"));
    }
    let resolved = path
        .canonicalize()
        .map_err(|_| error("Cannot locate configured inventory file"))?;
    if !resolved.starts_with(root) {
        return Err(error("Inventory file must remain inside its workspace"));
    }
    let file = File::open(&resolved).map_err(|_| error("Cannot read configured inventory file"))?;
    if !file
        .metadata()
        .map_err(|_| error("Cannot read configured inventory file"))?
        .is_file()
    {
        return Err(error("Inventory source must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take((MAX_INVENTORY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| error("Cannot read configured inventory file"))?;
    if bytes.len() > MAX_INVENTORY_BYTES {
        return Err(error("Inventory exceeds the 8 MiB limit"));
    }
    let actual = digest(&bytes);
    if actual != settings.sha256 {
        return Err(error(
            "Inventory digest differs from inventory.sha256; review the file and update its pin explicitly",
        ));
    }
    decode(&provider.id, &bytes, settings.max_age_seconds, now)
}
#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
