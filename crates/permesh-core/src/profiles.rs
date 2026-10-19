// SPDX-License-Identifier: MIT
//! Expected access (profiles) against observed access, per canonical identity.
//! Missing is claimed only where the instance was completely observed; unexplained
//! access keeps its full route and is a review finding, not proof of wrong access.
use crate::identity::IdentityIndex;
use crate::traversal::{MembershipGraph, paths, sort_paths};
use crate::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpectedEntry {
    Group {
        instance: String,
        group: String,
    },
    Role {
        instance: String,
        role: String,
        resource: String,
    },
}
#[derive(Clone, Debug, Default)]
pub struct ProfileInput {
    pub profiles: BTreeMap<String, BTreeSet<ExpectedEntry>>,
    pub people: BTreeMap<String, BTreeSet<String>>,
    pub directory_groups: Vec<DirectoryAssignment>,
    /// Completely observed instances and their declared capabilities.
    pub observable: BTreeMap<String, BTreeSet<String>>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DirectoryAssignment {
    pub instance: String,
    pub group: String,
    pub profiles: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    Finding,
    CannotTell,
}
#[derive(Clone, Debug, Serialize)]
pub struct MissingEntry {
    pub entry: ExpectedEntry,
    pub state: EntryState,
    pub reason: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct UnexplainedPath {
    pub path: AccessPath,
    pub state: EntryState,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonState {
    Compared,
    NotFound,
    Ambiguous,
}
#[derive(Clone, Debug, Serialize)]
pub struct IdentityComparison {
    pub identity: String,
    pub state: ComparisonState,
    pub profiles: Vec<String>,
    pub accounts: Vec<EntityKey>,
    pub explained: usize,
    pub missing: Vec<MissingEntry>,
    pub unexplained: Vec<UnexplainedPath>,
}
#[derive(Clone, Debug, Serialize)]
pub struct UnresolvedAssignment {
    pub instance: String,
    pub group: String,
    pub reason: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProfileComparison {
    pub identities: Vec<IdentityComparison>,
    pub not_covered: usize,
    pub ambiguous_accounts: usize,
    pub unresolved_members: usize,
    pub unresolved_assignments: Vec<UnresolvedAssignment>,
    pub complete: bool,
}

fn observable(input: &ProfileInput, instance: &str, capability: &str) -> bool {
    input
        .observable
        .get(instance)
        .is_some_and(|caps| caps.contains(capability))
}
/// Role entries also need membership coverage when the instance has groups,
/// because a derived path could otherwise satisfy them unseen.
fn roles_observable(input: &ProfileInput, instance: &str) -> bool {
    observable(input, instance, "grants")
        && (observable(input, instance, "memberships") || !observable(input, instance, "groups"))
}

pub fn compare_profiles(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    input: &ProfileInput,
) -> Result<ProfileComparison, DomainError> {
    let index = IdentityIndex::build(snapshots, aliases)?;
    let graph = MembershipGraph::new(snapshots);
    let groups: BTreeSet<&EntityKey> = snapshots
        .iter()
        .flat_map(|s| s.groups.iter().map(|g| &g.key))
        .collect();
    let mut assigned: BTreeMap<String, BTreeSet<String>> = input.people.clone();
    let mut unresolved_assignments = Vec::new();
    let mut unresolved_members = 0usize;
    for mapping in &input.directory_groups {
        let group = EntityKey::new(&mapping.instance, &mapping.group);
        let reason = if !observable(input, &mapping.instance, "memberships") {
            Some("instance_not_observable")
        } else if !groups.contains(&group) {
            Some("group_not_observed")
        } else {
            None
        };
        if let Some(reason) = reason {
            unresolved_assignments.push(UnresolvedAssignment {
                instance: mapping.instance.clone(),
                group: mapping.group.clone(),
                reason,
            });
            continue;
        }
        for key in index.accounts.keys() {
            if key.provider != mapping.instance || !graph.reachable(key)?.contains(&group) {
                continue;
            }
            match index.resolve(key) {
                IdentityResolution::Resolved { identity } => assigned
                    .entry(identity.id)
                    .or_default()
                    .extend(mapping.profiles.iter().cloned()),
                _ => unresolved_members += 1,
            }
        }
    }
    let assignments_complete = unresolved_assignments.is_empty();
    // Accounts attributed to exactly one canonical identity.
    let mut owned: BTreeMap<&str, Vec<&Account>> = BTreeMap::new();
    let mut ambiguous_accounts = 0usize;
    for (key, account) in &index.accounts {
        match index.candidates.get(key) {
            Some(ids) if ids.len() == 1 => {
                if let Some(id) = ids.first() {
                    owned.entry(id.as_str()).or_default().push(*account);
                }
            }
            Some(_) => ambiguous_accounts += 1,
            None => {}
        }
    }
    let mut identities = Vec::new();
    let mut complete = assignments_complete;
    for (id, profiles) in &assigned {
        let mut comparison = IdentityComparison {
            identity: id.clone(),
            state: ComparisonState::Compared,
            profiles: profiles.iter().cloned().collect(),
            accounts: vec![],
            explained: 0,
            missing: vec![],
            unexplained: vec![],
        };
        if !index.identities.contains_key(id) {
            comparison.state = ComparisonState::NotFound;
        } else if index.conflicting.contains(id) {
            comparison.state = ComparisonState::Ambiguous;
        }
        if comparison.state != ComparisonState::Compared {
            complete = false;
            identities.push(comparison);
            continue;
        }
        let expected: BTreeSet<&ExpectedEntry> = profiles
            .iter()
            .filter_map(|p| input.profiles.get(p))
            .flatten()
            .collect();
        let accounts = owned.get(id.as_str()).cloned().unwrap_or_default();
        comparison.accounts = accounts.iter().map(|a| a.key.clone()).collect();
        let mut access = paths(snapshots, accounts.iter().copied(), false)?;
        sort_paths(&mut access);
        let mut member_of = BTreeSet::new();
        for account in &accounts {
            member_of.extend(graph.reachable(&account.key)?);
        }
        let explains = |path: &AccessPath, entry: &ExpectedEntry| match entry {
            ExpectedEntry::Group { instance, group } => path
                .groups
                .iter()
                .any(|g| g.key.provider == *instance && g.key.id == *group),
            ExpectedEntry::Role {
                instance,
                role,
                resource,
            } => {
                path.resource.key.provider == *instance
                    && path.resource.key.id == *resource
                    && path.grant.role == *role
            }
        };
        for path in access.iter() {
            if expected.iter().any(|e| explains(path, e)) {
                comparison.explained += 1;
            } else {
                let state = if assignments_complete {
                    EntryState::Finding
                } else {
                    EntryState::CannotTell
                };
                comparison.unexplained.push(UnexplainedPath {
                    path: path.clone(),
                    state,
                });
            }
        }
        for entry in expected {
            let (instance, satisfied, visible) = match entry {
                ExpectedEntry::Group { instance, group } => (
                    instance,
                    member_of.contains(&EntityKey::new(instance, group)),
                    observable(input, instance, "memberships"),
                ),
                ExpectedEntry::Role {
                    instance,
                    role,
                    resource,
                } => (
                    instance,
                    access.iter().any(|p| {
                        p.resource.key.provider == *instance
                            && p.resource.key.id == *resource
                            && p.grant.role == *role
                    }),
                    roles_observable(input, instance),
                ),
            };
            if satisfied {
                continue;
            }
            let (state, reason) = if !visible {
                (EntryState::CannotTell, "instance_not_observable")
            } else if !accounts.iter().any(|a| a.key.provider == *instance) {
                (EntryState::Finding, "no_mapped_account")
            } else {
                (EntryState::Finding, "not_observed")
            };
            comparison.missing.push(MissingEntry {
                entry: entry.clone(),
                state,
                reason,
            });
        }
        complete &= comparison
            .missing
            .iter()
            .all(|m| m.state == EntryState::Finding)
            && comparison
                .unexplained
                .iter()
                .all(|u| u.state == EntryState::Finding);
        identities.push(comparison);
    }
    let not_covered = index
        .identities
        .values()
        .filter(|i| {
            !assigned.contains_key(&i.id)
                && owned.contains_key(i.id.as_str())
                && !matches!(i.kind, IdentityKind::Service | IdentityKind::Bot)
        })
        .count();
    Ok(ProfileComparison {
        identities,
        not_covered,
        ambiguous_accounts,
        unresolved_members,
        unresolved_assignments,
        complete,
    })
}
