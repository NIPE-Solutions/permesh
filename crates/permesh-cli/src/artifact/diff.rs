// SPDX-License-Identifier: MIT
//! Compare recorded observations. Absence is never proof of revoked authorization.
use super::{Artifact, ProviderCapture, State, invalid};
use crate::error::AppError;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
const MAX_CHANGES: usize = 100_000;
const MAX_CHANGE_BYTES: usize = 64 * 1024 * 1024;
#[derive(Serialize)]
pub struct Comparison {
    pub comparison_version: u32,
    pub identity_context_changed: bool,
    pub before_window: [String; 2],
    pub after_window: [String; 2],
    pub changes: Vec<Change>,
    pub inconclusive: Vec<Gap>,
    pub limitations: Vec<&'static str>,
}
#[derive(Serialize)]
pub struct Gap {
    pub instance: String,
    pub reason: &'static str,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    NewlyObserved,
    NoLongerObserved,
    Changed,
    MissingUnconfirmed,
}
#[derive(Serialize)]
pub struct Change {
    pub instance: String,
    pub entity_kind: &'static str,
    pub id: String,
    pub change: ChangeKind,
    pub before: Option<Value>,
    pub after: Option<Value>,
}
pub(crate) fn comparable(a: &ProviderCapture, b: &ProviderCapture) -> bool {
    a.state == State::Complete
        && b.state == State::Complete
        && a.context_sha256 == b.context_sha256
        && a.executable_sha256 == b.executable_sha256
        && a.provider_type == b.provider_type
        && a.provider_version == b.provider_version
        && a.capabilities == b.capabilities
        && a.configured_scope == b.configured_scope
        && a.limitations == b.limitations
        && match (&a.source_observation, &b.source_observation) {
            (None, None) => true,
            (
                Some(super::SourceObservation::FileInventory {
                    declared_scope: a, ..
                }),
                Some(super::SourceObservation::FileInventory {
                    declared_scope: b, ..
                }),
            ) => a == b,
            _ => false,
        }
}
pub(crate) fn ordered(a: &ProviderCapture, b: &ProviderCapture) -> bool {
    let parse = |s: &str| {
        time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
    };
    if parse(&a.completed_at)
        .zip(parse(&b.started_at))
        .is_none_or(|(a, b)| a > b)
    {
        return false;
    }
    match (&a.source_observation, &b.source_observation) {
        (None, None) => true,
        (
            Some(super::SourceObservation::FileInventory {
                exported_at: a,
                content_sha256: ah,
                ..
            }),
            Some(super::SourceObservation::FileInventory {
                exported_at: b,
                content_sha256: bh,
                ..
            }),
        ) => parse(a)
            .zip(parse(b))
            .is_some_and(|(a, b)| if ah == bh { a == b } else { a < b }),
        _ => false,
    }
}
fn semantic(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(k, _)| k.as_str() != "observed_at")
                .map(|(k, v)| (k.clone(), semantic(v)))
                .collect(),
        ),
        Value::Array(v) => Value::Array(v.iter().map(semantic).collect()),
        _ => value.clone(),
    }
}
fn records(
    p: Option<&ProviderCapture>,
) -> Result<BTreeMap<(&'static str, String), Value>, AppError> {
    let mut result = BTreeMap::new();
    let Some(data) = p.and_then(|p| p.data.as_ref()) else {
        return Ok(result);
    };
    // Serialize storage-owned DTOs only. Source IDs are compared without label matching.
    let value = serde_json::to_value(data).map_err(|_| invalid())?;
    for kind in [
        "identities",
        "accounts",
        "resources",
        "groups",
        "memberships",
        "grants",
    ] {
        let rows = value[kind].as_array().ok_or_else(invalid)?;
        for row in rows {
            let key = match kind {
                "identities" | "grants" => row["id"].clone(),
                "memberships" => serde_json::json!([row["member"], row["group"]]),
                _ => row["key"].clone(),
            };
            let id = serde_json::to_string(&key).map_err(|_| invalid())?;
            if result.insert((kind, id), row.clone()).is_some() {
                return Err(invalid());
            }
        }
    }
    Ok(result)
}
pub fn compare(before: &Artifact, after: &Artifact) -> Result<Comparison, AppError> {
    before.validate()?;
    after.validate()?;
    let a: BTreeMap<_, _> = before
        .providers
        .iter()
        .map(|p| (p.instance.as_str(), p))
        .collect();
    let b: BTreeMap<_, _> = after
        .providers
        .iter()
        .map(|p| (p.instance.as_str(), p))
        .collect();
    let ids: BTreeSet<_> = a.keys().chain(b.keys()).copied().collect();
    let mut result = Comparison {
        comparison_version: 1,
        identity_context_changed: before.identity != after.identity,
        before_window: [before.started_at.clone(), before.completed_at.clone()],
        after_window: [after.started_at.clone(), after.completed_at.clone()],
        changes: vec![],
        inconclusive: vec![],
        limitations: vec![
            "No longer observed means absent within a comparable visible collection, not revoked or denied access.",
            "Unreported upstream visibility changes cannot be detected from snapshots; provider APIs are not transactional.",
        ],
    };
    let mut change_bytes = 0usize;
    for id in ids {
        let old = a.get(id).copied();
        let new = b.get(id).copied();
        let mut can_compare = old.zip(new).is_some_and(|(old, new)| comparable(old, new));
        if !can_compare {
            result.inconclusive.push(Gap {
                instance: id.into(),
                reason: if old.is_none() || new.is_none() {
                    "provider_set_changed"
                } else {
                    "collection_or_context_not_comparable"
                },
            });
        }
        let authority_changed = before.identity.authorities.contains(&id.to_owned())
            != after.identity.authorities.contains(&id.to_owned());
        if authority_changed {
            result.inconclusive.push(Gap {
                instance: id.into(),
                reason: "identity_authority_changed",
            });
        }
        let old_records = records(old)?;
        let new_records = records(new)?;
        let keys: BTreeSet<_> = old_records
            .keys()
            .chain(new_records.keys())
            .cloned()
            .collect();
        let ordered = old.zip(new).is_some_and(|(a, b)| ordered(a, b));
        if !ordered {
            if keys.iter().any(|k| {
                !(k.0 == "identities" && authority_changed)
                    && old_records.get(k).map(semantic) != new_records.get(k).map(semantic)
            }) {
                result.inconclusive.push(Gap {
                    instance: id.into(),
                    reason: "capture_windows_not_ordered",
                });
            }
            can_compare = false;
        }
        for (kind, key) in keys {
            if kind == "identities" && authority_changed {
                continue;
            }
            let lookup = (kind, key.clone());
            let old = old_records.get(&lookup);
            let new = new_records.get(&lookup);
            let change = match (old, new) {
                (Some(a), Some(b)) if semantic(a) == semantic(b) => continue,
                (Some(_), Some(_)) => ChangeKind::Changed,
                (None, Some(_)) => ChangeKind::NewlyObserved,
                (Some(_), None) if can_compare => ChangeKind::NoLongerObserved,
                (Some(_), None) => ChangeKind::MissingUnconfirmed,
                (None, None) => continue,
            };
            let size = old
                .into_iter()
                .chain(new)
                .try_fold(key.len(), |sum, value| {
                    serde_json::to_vec(value).map(|v| sum.saturating_add(v.len()))
                })
                .map_err(|_| invalid())?;
            change_bytes = change_bytes.saturating_add(size);
            if result.changes.len() >= MAX_CHANGES || change_bytes > MAX_CHANGE_BYTES {
                return Err(AppError::input(
                    "Snapshot comparison exceeds the change budget; narrow the configured collection scope",
                ));
            }
            result.changes.push(Change {
                instance: id.into(),
                entity_kind: kind,
                id: key,
                change,
                before: old.cloned(),
                after: new.cloned(),
            });
        }
    }
    Ok(result)
}
