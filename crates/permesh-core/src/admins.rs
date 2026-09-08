// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::identity::IdentityIndex;
use crate::traversal::{MAX_PATH_STEPS, paths, sort_paths};
use crate::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize)]
pub struct AdminAccess {
    pub accounts: Vec<AdminAccount>,
    pub access: Vec<AccessPath>,
    pub unknown_access: Vec<AccessPath>,
    pub unresolved_grants: Vec<UnresolvedGrant>,
}
#[derive(Clone, Debug, Serialize)]
pub struct AdminAccount {
    pub account: Account,
    pub identity: IdentityResolution,
}
#[derive(Clone, Debug, Serialize)]
pub struct UnresolvedGrant {
    pub grant: Grant,
    pub resource: Resource,
    pub group: Option<Group>,
}

/// Inspect elevated, admin, owner, and uncertain access without requiring a
/// canonical identity. Standard grants do not contribute paths or accounts.
pub fn query_admins(snapshots: &[Snapshot], aliases: &Aliases) -> Result<AdminAccess, DomainError> {
    let index = IdentityIndex::build(snapshots, aliases)?;
    let mut selected_paths = paths(snapshots, index.accounts.values().copied(), true)?;
    sort_paths(&mut selected_paths);
    let observed_grants: BTreeSet<_> = selected_paths
        .iter()
        .map(|p| (p.resource.key.provider.clone(), p.grant.id.clone()))
        .collect();
    let account_keys: BTreeSet<_> = selected_paths.iter().map(|p| p.account.clone()).collect();
    let accounts = account_keys
        .iter()
        .map(|key| {
            let mut account = index.accounts[key].clone();
            account.verified_emails.sort();
            account.verified_emails.dedup();
            AdminAccount {
                account,
                identity: index.resolve(key),
            }
        })
        .collect();
    let (unknown_access, access) = selected_paths
        .into_iter()
        .partition(|p| p.grant.privilege == Privilege::Unknown);
    let mut unresolved_grants = Vec::new();
    for snapshot in snapshots {
        let resources: BTreeMap<_, _> = snapshot.resources.iter().map(|r| (&r.key, r)).collect();
        let groups: BTreeMap<_, _> = snapshot.groups.iter().map(|g| (&g.key, g)).collect();
        for grant in &snapshot.grants {
            if grant.privilege == Privilege::Standard
                || observed_grants.contains(&(snapshot.provider.clone(), grant.id.clone()))
            {
                continue;
            }
            if let Subject::Group(key) = &grant.subject {
                if unresolved_grants.len() >= MAX_PATH_STEPS {
                    return Err(DomainError::PathLimit);
                }
                unresolved_grants.push(UnresolvedGrant {
                    grant: grant.clone(),
                    resource: (*resources
                        .get(&grant.resource)
                        .ok_or(DomainError::Reference)?)
                    .clone(),
                    group: groups.get(key).map(|group| (*group).clone()),
                });
            }
        }
    }
    unresolved_grants
        .sort_by(|a, b| (&a.resource.key, &a.grant.id).cmp(&(&b.resource.key, &b.grant.id)));
    Ok(AdminAccess {
        accounts,
        access,
        unknown_access,
        unresolved_grants,
    })
}
