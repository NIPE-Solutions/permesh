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
    /// People who left: compared against no profiles, whatever assigns them.
    pub left: BTreeSet<String>,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
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

pub(crate) fn observable(input: &ProfileInput, instance: &str, capability: &str) -> bool {
    input
        .observable
        .get(instance)
        .is_some_and(|caps| caps.contains(capability))
}
/// Role entries also need membership coverage when the instance has groups,
/// because a derived path could otherwise satisfy them unseen.
pub(crate) fn roles_observable(input: &ProfileInput, instance: &str) -> bool {
    observable(input, instance, "grants")
        && (observable(input, instance, "memberships") || !observable(input, instance, "groups"))
}

/// A path is explained by a group entry it passes through, or by a role entry it ends in.
pub(crate) fn explains(path: &AccessPath, entry: &ExpectedEntry) -> bool {
    match entry {
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
    }
}

/// Accounts attributed to exactly one canonical identity, the instances where an
/// identity also claims an ambiguous account, and how many accounts are ambiguous.
pub(crate) type Owned<'a> = (
    BTreeMap<&'a str, Vec<&'a Account>>,
    BTreeMap<&'a str, BTreeSet<&'a str>>,
    usize,
);
pub(crate) fn owned_accounts<'a>(index: &'a IdentityIndex<'a>) -> Owned<'a> {
    let mut owned: BTreeMap<&str, Vec<&Account>> = BTreeMap::new();
    let mut claimed: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut ambiguous = 0usize;
    for (key, account) in &index.accounts {
        match index.candidates.get(key) {
            Some(ids) if ids.len() == 1 => {
                if let Some(id) = ids.first() {
                    owned.entry(id.as_str()).or_default().push(*account);
                }
            }
            Some(ids) => {
                ambiguous += 1;
                for id in ids {
                    claimed
                        .entry(id.as_str())
                        .or_default()
                        .insert(key.provider.as_str());
                }
            }
            None => {}
        }
    }
    (owned, claimed, ambiguous)
}

/// Effective profile assignment: people files plus directory group mappings.
#[derive(Clone, Debug, Default)]
pub struct Assignments {
    pub profiles: BTreeMap<String, BTreeSet<String>>,
    /// Mappings whose directory group could not be read completely.
    pub unresolved_assignments: Vec<UnresolvedAssignment>,
    /// Directory group members without exactly one canonical identity.
    pub unresolved_members: usize,
}
impl Assignments {
    /// Incomplete directory observation leaves assignments unresolved, not empty.
    pub fn complete(&self) -> bool {
        self.unresolved_assignments.is_empty() && self.unresolved_members == 0
    }
}
pub fn assign_profiles(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    input: &ProfileInput,
) -> Result<Assignments, DomainError> {
    let index = IdentityIndex::build(snapshots, aliases)?;
    assign(snapshots, &index, &MembershipGraph::new(snapshots), input)
}
fn assign(
    snapshots: &[Snapshot],
    index: &IdentityIndex<'_>,
    graph: &MembershipGraph<'_>,
    input: &ProfileInput,
) -> Result<Assignments, DomainError> {
    let groups: BTreeSet<&EntityKey> = snapshots
        .iter()
        .flat_map(|s| s.groups.iter().map(|g| &g.key))
        .collect();
    let mut assigned: BTreeMap<String, BTreeSet<String>> = input.people.clone();
    let mut unresolved_assignments = Vec::new();
    let mut unresolved_members = BTreeSet::new();
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
                _ => {
                    unresolved_members.insert(key);
                }
            }
        }
    }
    Ok(Assignments {
        profiles: assigned,
        unresolved_assignments,
        unresolved_members: unresolved_members.len(),
    })
}

pub fn compare_profiles(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    input: &ProfileInput,
) -> Result<ProfileComparison, DomainError> {
    let index = IdentityIndex::build(snapshots, aliases)?;
    let graph = MembershipGraph::new(snapshots);
    let Assignments {
        profiles: mut assigned,
        unresolved_assignments,
        unresolved_members,
    } = assign(snapshots, &index, &graph, input)?;
    // Someone who left is expected to hold nothing: remaining access is unexplained,
    // and their former profiles are not missing.
    for identity in &input.left {
        assigned.insert(identity.clone(), BTreeSet::new());
    }
    let assignments_complete = unresolved_assignments.is_empty();
    let (owned, claimed, ambiguous_accounts) = owned_accounts(&index);
    let mut identities = Vec::new();
    // Assigned members without a resolved identity were never compared.
    let mut complete = assignments_complete && unresolved_members == 0;
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
            } else if claimed
                .get(id.as_str())
                .is_some_and(|instances| instances.contains(instance.as_str()))
            {
                // An account this person may own was not attributed; it could hold the entry.
                (EntryState::CannotTell, "ambiguous_account")
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
        // Access of an unattributed account is hidden from both claimants.
        complete &= !claimed.contains_key(id.as_str());
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

#[derive(Clone, Debug, Serialize)]
pub struct SuggestedEntry {
    pub entry: ExpectedEntry,
    /// Display name only; never a key.
    pub label: String,
    pub count: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Suggestion {
    pub group: Group,
    pub members: usize,
    pub unresolved_members: usize,
    pub entries: Vec<SuggestedEntry>,
}
fn label(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).collect()
}

/// Draft profile entries shared by the resolved members of one group (direct or
/// nested): group memberships first, then roles no suggested group explains.
pub fn suggest_profile(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    group: &EntityKey,
    min_percent: u8,
) -> Result<Suggestion, DomainError> {
    if min_percent == 0 || min_percent > 100 {
        return Err(DomainError::Identifier);
    }
    let selected_group = snapshots
        .iter()
        .flat_map(|s| s.groups.iter())
        .find(|g| g.key == *group)
        .ok_or(DomainError::NotFound)?
        .clone();
    let index = IdentityIndex::build(snapshots, aliases)?;
    let graph = MembershipGraph::new(snapshots);
    let (owned, _, _) = owned_accounts(&index);
    let mut members = BTreeSet::new();
    let mut unresolved_members = 0usize;
    for key in index.accounts.keys() {
        if !graph.reachable(key)?.contains(group) {
            continue;
        }
        match index.resolve(key) {
            IdentityResolution::Resolved { identity } => {
                members.insert(identity.id);
            }
            _ => unresolved_members += 1,
        }
    }
    let total = members.len();
    let keep = |count: usize| count * 100 >= usize::from(min_percent) * total;
    let names: BTreeMap<&EntityKey, &str> = snapshots
        .iter()
        .flat_map(|s| {
            s.groups
                .iter()
                .map(|g| (&g.key, g.name.as_str()))
                .chain(s.resources.iter().map(|r| (&r.key, r.name.as_str())))
        })
        .collect();
    let mut group_counts: BTreeMap<EntityKey, usize> = BTreeMap::new();
    let mut member_paths = Vec::new();
    for id in &members {
        let accounts = owned.get(id.as_str()).cloned().unwrap_or_default();
        let mut reached = BTreeSet::new();
        for account in &accounts {
            reached.extend(graph.reachable(&account.key)?);
        }
        for key in reached {
            *group_counts.entry(key).or_default() += 1;
        }
        member_paths.push(paths(snapshots, accounts.iter().copied(), false)?);
    }
    let chosen: BTreeSet<EntityKey> = group_counts
        .iter()
        .filter(|(_, count)| keep(**count))
        .map(|(key, _)| key.clone())
        .collect();
    let mut role_counts: BTreeMap<(EntityKey, String), usize> = BTreeMap::new();
    for access in &member_paths {
        let held: BTreeSet<(EntityKey, String)> = access
            .iter()
            .filter(|p| !p.groups.iter().any(|g| chosen.contains(&g.key)))
            .map(|p| (p.resource.key.clone(), p.grant.role.clone()))
            .collect();
        for role in held {
            *role_counts.entry(role).or_default() += 1;
        }
    }
    let name = |key: &EntityKey| label(names.get(key).copied().unwrap_or_default());
    let mut entries: Vec<SuggestedEntry> = chosen
        .iter()
        .map(|key| SuggestedEntry {
            entry: ExpectedEntry::Group {
                instance: key.provider.clone(),
                group: key.id.clone(),
            },
            label: name(key),
            count: group_counts[key],
        })
        .collect();
    entries.extend(
        role_counts
            .into_iter()
            .filter(|(_, count)| keep(*count))
            .map(|((resource, role), count)| SuggestedEntry {
                label: name(&resource),
                entry: ExpectedEntry::Role {
                    instance: resource.provider,
                    role,
                    resource: resource.id,
                },
                count,
            }),
    );
    Ok(Suggestion {
        group: selected_group,
        members: total,
        unresolved_members,
        entries,
    })
}
