// SPDX-License-Identifier: MIT
use crate::identity::IdentityIndex;
use crate::traversal::{paths, sort_paths};
use crate::*;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize)]
pub struct UserAccess {
    pub identity: Option<Identity>,
    pub accounts: Vec<Account>,
    pub access: Vec<AccessPath>,
}
#[derive(Clone, Debug, Serialize)]
pub struct AccessPath {
    pub account: EntityKey,
    pub groups: Vec<Group>,
    pub memberships: Vec<Membership>,
    pub resource: Resource,
    pub grant: Grant,
}

impl AccessPath {
    /// A membership path derives reachability from the original grant evidence.
    /// It does not establish that the reported permission is effective access.
    pub fn certainty(&self) -> Certainty {
        if self.grant.certainty == Certainty::Observed
            && (!self.groups.is_empty() || !self.memberships.is_empty())
        {
            Certainty::Derived
        } else {
            self.grant.certainty
        }
    }
}

/// Query one identity or account using exact, verified matching.
pub fn query_user(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    query: &str,
) -> Result<UserAccess, DomainError> {
    let IdentityIndex {
        identities,
        conflicting,
        email_index,
        accounts,
        candidates,
    } = IdentityIndex::build(snapshots, aliases)?;
    let mut target_identities = BTreeSet::new();
    if identities.contains_key(query) {
        target_identities.insert(query.to_string());
    }
    if let Some(matches) = email_index.get(query) {
        target_identities.extend(matches.iter().cloned());
    }
    let direct_accounts: Vec<_> = accounts
        .values()
        .filter(|a| a.login == query || format!("{}:{}", a.key.provider, a.key.id) == query)
        .copied()
        .collect();
    if target_identities.is_empty() {
        if direct_accounts.len() > 1 {
            return Err(DomainError::Ambiguous);
        }
        if let Some(account) = direct_accounts.first()
            && let Some(matches) = candidates.get(&account.key)
        {
            target_identities.extend(matches.iter().cloned());
        }
    }
    if target_identities.len() > 1 {
        return Err(DomainError::Ambiguous);
    }
    let target = target_identities.first();
    if target.is_some_and(|id| conflicting.contains(id)) {
        return Err(DomainError::Ambiguous);
    }
    let mut selected: Vec<Account> = if let Some(id) = target {
        let mut selected = Vec::new();
        for (key, account) in &accounts {
            if let Some(matches) = candidates.get(key)
                && matches.contains(id)
            {
                if matches.len() != 1 {
                    return Err(DomainError::Ambiguous);
                }
                selected.push((*account).clone());
            }
        }
        selected
    } else {
        direct_accounts.into_iter().cloned().collect()
    };
    if selected.is_empty() && target.is_none() {
        return Err(DomainError::NotFound);
    }
    for account in &mut selected {
        account.verified_emails.sort();
        account.verified_emails.dedup();
    }
    let mut access = paths(snapshots, &selected, false)?;
    sort_paths(&mut access);
    Ok(UserAccess {
        identity: target.and_then(|id| identities.get(id)).cloned(),
        accounts: selected,
        access,
    })
}
