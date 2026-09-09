// SPDX-License-Identifier: MIT
//! Resource-first evidence, preserving original grants and each bounded membership path.
use crate::{
    identity::IdentityIndex,
    traversal::{MAX_PATH_STEPS, paths_for_resource, sort_paths},
    *,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct ResourceAccess {
    pub resource: Resource,
    pub accounts: Vec<AdminAccount>,
    pub access: Vec<AccessPath>,
    /// Unique source grants, not a count of effective permissions.
    pub grants: Vec<Grant>,
    /// Group grants with no observed account path, or incomplete provider visibility.
    pub unresolved_grants: Vec<UnresolvedGrant>,
    pub provider_complete: bool,
}
pub fn query_resource(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    authorities: &[String],
    key: &EntityKey,
) -> Result<ResourceAccess, DomainError> {
    let authority_set = authorities.iter().map(String::as_str).collect();
    let index = IdentityIndex::build_with_authorities(snapshots, aliases, &authority_set)?;
    let snapshot = snapshots
        .iter()
        .find(|s| s.provider == key.provider)
        .ok_or(DomainError::NotFound)?;
    let resource = snapshot
        .resources
        .iter()
        .find(|r| r.key == *key)
        .ok_or(DomainError::NotFound)?
        .clone();
    let mut access = paths_for_resource(snapshots, index.accounts.values().copied(), key)?;
    sort_paths(&mut access);
    let account_keys: BTreeSet<_> = access.iter().map(|p| &p.account).collect();
    let mut account_budget = crate::path_budget::CopyBudget::new();
    let accounts = account_keys
        .iter()
        .map(|key| {
            let mut account = index.accounts[*key].clone();
            account.verified_emails.sort();
            account.verified_emails.dedup();
            let identity = index.resolve(key);
            let strings =
                |values: &[String]| values.iter().fold(0usize, |n, s| n.saturating_add(s.len()));
            let identity_bytes = match &identity {
                IdentityResolution::Resolved { identity } => identity
                    .id
                    .len()
                    .saturating_add(strings(&identity.verified_emails)),
                IdentityResolution::Ambiguous { candidates } => strings(candidates),
                IdentityResolution::Unmapped => 0,
            };
            account_budget.charge(
                size_of::<AdminAccount>()
                    .saturating_add(crate::path_budget::key_heap(&account.key))
                    .saturating_add(account.login.len())
                    .saturating_add(strings(&account.verified_emails))
                    .saturating_add(identity_bytes),
            )?;
            Ok(AdminAccount { account, identity })
        })
        .collect::<Result<Vec<_>, DomainError>>()?;
    let observed: BTreeSet<_> = access.iter().map(|p| p.grant.id.as_str()).collect();
    let groups: BTreeMap<_, _> = snapshot.groups.iter().map(|g| (&g.key, g)).collect();
    let mut grants: Vec<_> = snapshot
        .grants
        .iter()
        .filter(|g| g.resource == *key)
        .collect();
    if grants.len() > MAX_PATH_STEPS {
        return Err(DomainError::PathLimit);
    }
    grants.sort_by(|a, b| a.id.cmp(&b.id));
    let mut budget = crate::path_budget::CopyBudget::new();
    let mut unresolved_grants = Vec::new();
    for grant in &grants {
        budget.charge(crate::path_budget::grant(grant))?;
        if let Subject::Group(group_key) = &grant.subject
            && (!snapshot.complete || !observed.contains(grant.id.as_str()))
        {
            let group = groups.get(group_key);
            budget.charge(
                crate::path_budget::grant(grant)
                    .saturating_add(crate::path_budget::resource(&resource))
                    .saturating_add(group.map_or(0, |g| crate::path_budget::group(g))),
            )?;
            unresolved_grants.push(UnresolvedGrant {
                grant: (*grant).clone(),
                resource: resource.clone(),
                group: group.map(|g| (*g).clone()),
            });
        }
    }
    Ok(ResourceAccess {
        resource,
        accounts,
        access,
        grants: grants.into_iter().cloned().collect(),
        unresolved_grants,
        provider_complete: snapshot.complete,
    })
}
