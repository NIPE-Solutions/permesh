// SPDX-License-Identifier: MIT
//! Deterministic, explicit identity mapping review. Labels never become evidence.
use crate::{identity::IdentityIndex, *};
use std::collections::{BTreeMap, BTreeSet};

/// Avoid unbounded report/mapping work across otherwise individually valid snapshots.
pub const MAX_REVIEW_ACCOUNTS: usize = 100_000;
#[derive(Clone, Debug)]
pub struct CanonicalIdentity {
    pub identity: Identity,
    pub sources: Vec<String>,
    pub conflicting: bool,
}
#[derive(Clone, Debug)]
pub struct AccountEvidence {
    pub account: Account,
    pub resolution: IdentityResolution,
    pub explicit_mappings: Vec<String>,
    pub verified_matches: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct IdentityReview {
    pub accounts: Vec<AccountEvidence>,
    pub identities: Vec<CanonicalIdentity>,
}
#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("mapping changes require complete fresh observations from every configured source")]
    Incomplete,
    #[error(
        "mapping target must be an exact, observed canonical identity from an authoritative source"
    )]
    TargetMissing,
    #[error(
        "mapping conflicts with existing mappings or authoritative identity evidence; inspect the account before changing it"
    )]
    Conflict,
    #[error("account must be identified by its observed provider instance and immutable native ID")]
    AccountMissing,
    #[error("the exact explicit mapping does not exist")]
    MappingMissing,
    #[error("the exact explicit mapping already exists")]
    Unchanged,
}

pub fn review(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    authorities: &[String],
) -> Result<IdentityReview, DomainError> {
    if snapshots.iter().fold(0usize, |total, s| {
        total
            .saturating_add(s.accounts.len())
            .saturating_add(s.identities.len())
    }) > MAX_REVIEW_ACCOUNTS
    {
        return Err(DomainError::Limit);
    }
    let authority_set = authorities
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let authoritative =
        IdentityIndex::build_with_authorities(snapshots, &Aliases::new(), &authority_set)?;
    let combined = IdentityIndex::build_with_authorities(snapshots, aliases, &authority_set)?;
    let mut sources: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for snapshot in snapshots
        .iter()
        .filter(|s| authority_set.contains(s.provider.as_str()))
    {
        for identity in &snapshot.identities {
            sources
                .entry(&identity.id)
                .or_default()
                .insert(&snapshot.provider);
        }
    }
    let mut explicit: BTreeMap<EntityKey, Vec<String>> = BTreeMap::new();
    for (identity, instances) in aliases {
        for (provider, ids) in instances {
            for id in ids {
                explicit
                    .entry(EntityKey::new(provider, id))
                    .or_default()
                    .push(identity.clone());
            }
        }
    }
    let identities = authoritative
        .identities
        .values()
        .map(|identity| CanonicalIdentity {
            identity: identity.clone(),
            sources: sources
                .get(identity.id.as_str())
                .map(|items| items.iter().map(|s| (*s).to_owned()).collect())
                .unwrap_or_default(),
            conflicting: authoritative.conflicting.contains(&identity.id),
        })
        .collect();
    let accounts = combined
        .accounts
        .iter()
        .map(|(key, account)| {
            let mut account = (*account).clone();
            account.verified_emails.sort();
            account.verified_emails.dedup();
            AccountEvidence {
                account,
                resolution: combined.resolve(key),
                explicit_mappings: explicit.remove(key).unwrap_or_default(),
                verified_matches: authoritative
                    .candidates
                    .get(key)
                    .map(|ids| ids.iter().cloned().collect())
                    .unwrap_or_default(),
            }
        })
        .collect();
    Ok(IdentityReview {
        accounts,
        identities,
    })
}

pub fn change(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    authorities: &[String],
    account: &EntityKey,
    identity: &str,
    remove: bool,
) -> Result<Aliases, MappingError> {
    if snapshots.iter().any(|s| !s.complete)
        || authorities
            .iter()
            .any(|id| !snapshots.iter().any(|s| s.provider == *id))
    {
        return Err(MappingError::Incomplete);
    }
    let before = review(snapshots, aliases, authorities)?;
    let mut proposed = aliases.clone();
    if remove {
        let bindings = proposed
            .get_mut(identity)
            .ok_or(MappingError::MappingMissing)?;
        let ids = bindings
            .get_mut(&account.provider)
            .ok_or(MappingError::MappingMissing)?;
        if !ids.contains(&account.id) {
            return Err(MappingError::MappingMissing);
        }
        ids.retain(|id| id != &account.id);
        if ids.is_empty() {
            bindings.remove(&account.provider);
        }
        if bindings.is_empty() {
            proposed.remove(identity);
        }
        return Ok(proposed);
    }
    let canonical = before
        .identities
        .iter()
        .find(|target| target.identity.id == identity)
        .ok_or(MappingError::TargetMissing)?;
    if canonical.conflicting {
        return Err(MappingError::Conflict);
    }
    let observed = before
        .accounts
        .iter()
        .find(|entry| entry.account.key == *account)
        .ok_or(MappingError::AccountMissing)?;
    if observed.explicit_mappings.iter().any(|id| id != identity) {
        return Err(MappingError::Conflict);
    }
    if observed.explicit_mappings.iter().any(|id| id == identity) {
        return Err(MappingError::Unchanged);
    }
    let ids = proposed
        .entry(identity.into())
        .or_default()
        .entry(account.provider.clone())
        .or_default();
    ids.push(account.id.clone());
    ids.sort();
    let after = review(snapshots, &proposed, authorities)?;
    if !after.accounts.iter().find(|entry| entry.account.key == *account).is_some_and(|entry| matches!(&entry.resolution, IdentityResolution::Resolved { identity: resolved } if resolved.id == identity)) { return Err(MappingError::Conflict); }
    Ok(proposed)
}
