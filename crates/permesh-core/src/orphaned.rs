// SPDX-License-Identifier: MIT
use crate::identity::IdentityIndex;
use crate::traversal::{paths, sort_paths};
use crate::*;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize)]
pub struct OrphanedAccess {
    pub authorities: Vec<String>,
    pub authority_complete: bool,
    pub accounts: Vec<OrphanedAccount>,
    pub access: Vec<AccessPath>,
}
#[derive(Clone, Debug, Serialize)]
pub struct OrphanedAccount {
    pub account: Account,
    pub identity: IdentityResolution,
    pub reason: OrphanReason,
}
/// Review categories: service, bot, external, and uncertain accounts are not
/// automatically orphan findings. Missing authority makes every account unassessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrphanReason {
    InactiveIdentity,
    SuspendedIdentity,
    UnknownIdentity,
    UnknownStatus,
    AmbiguousIdentity,
    ExternalIdentity,
    ServiceAccount,
    Bot,
    Unassessed,
}

/// Compare provider accounts with the named identity authorities using exact
/// identity resolution, retaining all observed access for selected accounts.
/// Accounts without grants remain visible; group grants without account paths
/// are outside this query and their absence does not establish group ownership.
pub fn query_orphaned(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    authority_ids: &[String],
) -> Result<OrphanedAccess, DomainError> {
    if authority_ids.is_empty() || authority_ids.iter().any(|id| id.trim().is_empty()) {
        return Err(DomainError::Identifier);
    }
    let authorities: BTreeSet<_> = authority_ids.iter().map(String::as_str).collect();
    let index = IdentityIndex::build_with_authorities(snapshots, aliases, &authorities)?;
    let complete: BTreeSet<_> = snapshots
        .iter()
        .filter(|s| s.complete)
        .map(|s| s.provider.as_str())
        .collect();
    let authority_complete = authorities.is_subset(&complete);
    let mut accounts = Vec::new();
    let mut selected = Vec::new();
    for (key, account) in &index.accounts {
        let identity = index.resolve(key);
        let reason = if authority_complete {
            classify(account, &identity)
        } else {
            Some(OrphanReason::Unassessed)
        };
        if let Some(reason) = reason {
            selected.push(*account);
            let mut account = (*account).clone();
            account.verified_emails.sort();
            account.verified_emails.dedup();
            accounts.push(OrphanedAccount {
                account,
                identity,
                reason,
            });
        }
    }
    let mut access = paths(snapshots, selected, false)?;
    sort_paths(&mut access);
    Ok(OrphanedAccess {
        authorities: authorities.into_iter().map(str::to_owned).collect(),
        authority_complete,
        accounts,
        access,
    })
}
fn classify(account: &Account, resolution: &IdentityResolution) -> Option<OrphanReason> {
    let (kind, affiliation, status) = match resolution {
        IdentityResolution::Ambiguous { .. } => return Some(OrphanReason::AmbiguousIdentity),
        IdentityResolution::Resolved { identity } => {
            (identity.kind, identity.affiliation, identity.status)
        }
        IdentityResolution::Unmapped => (
            IdentityKind::Unknown,
            Affiliation::Unknown,
            IdentityStatus::Unknown,
        ),
    };
    if status == IdentityStatus::Inactive {
        return Some(OrphanReason::InactiveIdentity);
    }
    if status == IdentityStatus::Suspended {
        return Some(OrphanReason::SuspendedIdentity);
    }
    if kind == IdentityKind::Bot || account.kind == IdentityKind::Bot {
        return Some(OrphanReason::Bot);
    }
    if kind == IdentityKind::Service || account.kind == IdentityKind::Service {
        return Some(OrphanReason::ServiceAccount);
    }
    if affiliation == Affiliation::External || account.affiliation == Affiliation::External {
        return Some(OrphanReason::ExternalIdentity);
    }
    match resolution {
        IdentityResolution::Resolved { .. } if status == IdentityStatus::Active => None,
        IdentityResolution::Resolved { .. } => Some(OrphanReason::UnknownStatus),
        IdentityResolution::Unmapped => Some(OrphanReason::UnknownIdentity),
        IdentityResolution::Ambiguous { .. } => Some(OrphanReason::AmbiguousIdentity),
    }
}
