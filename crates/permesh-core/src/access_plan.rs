// SPDX-License-Identifier: MIT
//! A change in someone's profiles (join, move, leave) as operation-shaped steps.
//! Pure planning over observations: every step is carried out by a person, and a
//! step that could not be fully observed stays `cannot_tell`.
use crate::identity::IdentityIndex;
use crate::profiles::{explains, observable, owned_accounts, roles_observable};
use crate::traversal::{MembershipGraph, paths, sort_paths};
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct PlanRequest {
    pub identity: String,
    /// Profiles the person holds after the change.
    pub after: BTreeSet<String>,
    /// Profiles taken away; their access goes unless an `after` profile justifies it.
    pub removed: BTreeSet<String>,
    /// Remove every observed or derived access path, justified or not.
    pub leave: bool,
}
/// Operations reference instances and immutable provider IDs, never labels.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    AccountNeeded {
        instance: String,
    },
    AddMembership {
        instance: String,
        group: String,
    },
    AddRole {
        instance: String,
        role: String,
        resource: String,
    },
    RemoveMembership {
        instance: String,
        account: String,
        group: String,
    },
    RemoveRole {
        instance: String,
        account: String,
        role: String,
        resource: String,
        grant_id: String,
    },
    ReviewAccountLifecycle {
        instance: String,
        account: String,
    },
    /// Leave plans only; produced from the offboarding assessment, never by `plan_access`.
    ReviewOwnership {
        instance: String,
        account: String,
        resource: String,
    },
    /// Leave plans only; produced from owner annotations, never by `plan_access`.
    ReviewMachineDependency {
        instance: String,
        account: String,
    },
}
impl Operation {
    pub fn instance(&self) -> &str {
        match self {
            Self::AccountNeeded { instance }
            | Self::AddMembership { instance, .. }
            | Self::AddRole { instance, .. }
            | Self::RemoveMembership { instance, .. }
            | Self::RemoveRole { instance, .. }
            | Self::ReviewAccountLifecycle { instance, .. }
            | Self::ReviewOwnership { instance, .. }
            | Self::ReviewMachineDependency { instance, .. } => instance,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Justification {
    Profiles(Vec<String>),
    Unexplained,
}
#[derive(Clone, Debug, Serialize)]
pub struct PlannedStep {
    pub operation: Operation,
    pub state: EntryState,
    pub reason: &'static str,
    pub justification: Justification,
    /// Grant IDs of the observed paths this step concerns.
    pub evidence: Vec<String>,
    /// Instance whose `account_needed` step must happen first.
    pub needs_account: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct KeptAccess {
    pub entry: ExpectedEntry,
    pub because: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct PlanResult {
    pub identity: String,
    pub state: ComparisonState,
    pub accounts: Vec<EntityKey>,
    pub steps: Vec<PlannedStep>,
    pub kept: Vec<KeptAccess>,
    pub complete: bool,
}

/// Steps keyed by operation, so each operation appears once with merged evidence.
#[derive(Default)]
struct Steps(BTreeMap<Operation, PlannedStep>);
impl Steps {
    fn add(&mut self, step: PlannedStep) {
        let Some(existing) = self.0.get_mut(&step.operation) else {
            self.0.insert(step.operation.clone(), step);
            return;
        };
        existing.evidence.extend(step.evidence);
        existing.evidence.sort();
        existing.evidence.dedup();
        if step.state == EntryState::CannotTell && existing.state != EntryState::CannotTell {
            existing.state = step.state;
            existing.reason = step.reason;
        }
        existing.justification = match (existing.justification.clone(), step.justification) {
            (Justification::Profiles(mut a), Justification::Profiles(b)) => {
                a.extend(b);
                a.sort();
                a.dedup();
                Justification::Profiles(a)
            }
            (Justification::Profiles(a), Justification::Unexplained)
            | (Justification::Unexplained, Justification::Profiles(a)) => {
                Justification::Profiles(a)
            }
            (Justification::Unexplained, Justification::Unexplained) => Justification::Unexplained,
        };
    }
}

fn sorted(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let set: BTreeSet<String> = names.into_iter().collect();
    set.into_iter().collect()
}

pub fn plan_access(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    input: &ProfileInput,
    request: &PlanRequest,
) -> Result<PlanResult, DomainError> {
    if request
        .after
        .iter()
        .chain(&request.removed)
        .any(|name| !input.profiles.contains_key(name))
    {
        return Err(DomainError::Identifier);
    }
    let index = IdentityIndex::build(snapshots, aliases)?;
    let (owned, claimed, _) = owned_accounts(&index);
    let id = request.identity.as_str();
    let mut result = PlanResult {
        identity: request.identity.clone(),
        state: if !index.identities.contains_key(id) {
            ComparisonState::NotFound
        } else if index.conflicting.contains(id) {
            ComparisonState::Ambiguous
        } else {
            ComparisonState::Compared
        },
        accounts: vec![],
        steps: vec![],
        kept: vec![],
        complete: false,
    };
    // Conflicting identity evidence: nothing about this person can be attributed.
    if result.state == ComparisonState::Ambiguous {
        return Ok(result);
    }
    let accounts = owned.get(id).cloned().unwrap_or_default();
    result.accounts = accounts.iter().map(|a| a.key.clone()).collect();
    // Instances where an account this person may own was not attributed to them.
    let claimed_in = claimed.get(id).cloned().unwrap_or_default();
    let mut access = paths(snapshots, accounts.iter().copied(), false)?;
    sort_paths(&mut access);
    let graph = MembershipGraph::new(snapshots);
    let mut member_of = BTreeSet::new();
    for account in &accounts {
        member_of.extend(graph.reachable(&account.key)?);
    }
    let mut after: BTreeMap<&ExpectedEntry, Vec<String>> = BTreeMap::new();
    for name in &request.after {
        for entry in &input.profiles[name] {
            after.entry(entry).or_default().push(name.clone());
        }
    }
    let mut steps = Steps::default();
    if !request.leave {
        let mut needs_account: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for (entry, profiles) in &after {
            let (instance, satisfied, visible, operation) = match entry {
                ExpectedEntry::Group { instance, group } => (
                    instance.as_str(),
                    member_of.contains(&EntityKey::new(instance, group)),
                    observable(input, instance, "memberships"),
                    Operation::AddMembership {
                        instance: instance.clone(),
                        group: group.clone(),
                    },
                ),
                ExpectedEntry::Role {
                    instance,
                    role,
                    resource,
                } => (
                    instance.as_str(),
                    access.iter().any(|p| {
                        p.resource.key.provider == *instance
                            && p.resource.key.id == *resource
                            && p.grant.role == *role
                    }),
                    roles_observable(input, instance),
                    Operation::AddRole {
                        instance: instance.clone(),
                        role: role.clone(),
                        resource: resource.clone(),
                    },
                ),
            };
            if satisfied {
                continue;
            }
            let has_account = accounts.iter().any(|a| a.key.provider == instance);
            let (state, reason, needs) = if !visible {
                (EntryState::CannotTell, "instance_not_observable", None)
            } else if claimed_in.contains(instance) {
                (EntryState::CannotTell, "ambiguous_account", None)
            } else if !has_account {
                (EntryState::Finding, "no_mapped_account", Some(instance))
            } else {
                (EntryState::Finding, "not_observed", None)
            };
            if let Some(instance) = needs {
                needs_account
                    .entry(instance)
                    .or_default()
                    .extend(profiles.iter().cloned());
            }
            steps.add(PlannedStep {
                operation,
                state,
                reason,
                justification: Justification::Profiles(profiles.clone()),
                evidence: vec![],
                needs_account: needs.map(str::to_owned),
            });
        }
        for (instance, profiles) in needs_account {
            steps.add(PlannedStep {
                operation: Operation::AccountNeeded {
                    instance: instance.to_owned(),
                },
                state: EntryState::Finding,
                reason: "no_mapped_account",
                justification: Justification::Profiles(sorted(profiles)),
                evidence: vec![],
                needs_account: None,
            });
        }
    }
    // Removals act on the first membership hop of each route: derived access is
    // planned at the membership that grants it, never as individual resource grants.
    let state_for = |instance: &str, visible: bool| {
        if !visible {
            (EntryState::CannotTell, "instance_not_observable")
        } else if claimed_in.contains(instance) {
            (EntryState::CannotTell, "ambiguous_account")
        } else {
            (EntryState::Finding, "observed")
        }
    };
    let remove_role = |path: &AccessPath, justification: Justification| {
        let instance = &path.resource.key.provider;
        let (state, reason) = state_for(instance, roles_observable(input, instance));
        PlannedStep {
            operation: Operation::RemoveRole {
                instance: instance.clone(),
                account: path.account.id.clone(),
                role: path.grant.role.clone(),
                resource: path.resource.key.id.clone(),
                grant_id: path.grant.id.clone(),
            },
            state,
            reason,
            justification,
            evidence: vec![path.grant.id.clone()],
            needs_account: None,
        }
    };
    let remove_membership =
        |account: &EntityKey, group: &EntityKey, evidence: Vec<String>, justification| {
            let (state, reason) = state_for(
                &group.provider,
                observable(input, &group.provider, "memberships"),
            );
            PlannedStep {
                operation: Operation::RemoveMembership {
                    instance: group.provider.clone(),
                    account: account.id.clone(),
                    group: group.id.clone(),
                },
                state,
                reason,
                justification,
                evidence,
                needs_account: None,
            }
        };
    let profiles_of = |names: &BTreeSet<String>| {
        let mut entries: BTreeMap<&ExpectedEntry, Vec<String>> = BTreeMap::new();
        for name in names {
            for entry in &input.profiles[name] {
                entries.entry(entry).or_default().push(name.clone());
            }
        }
        entries
    };
    let mut kept: BTreeMap<ExpectedEntry, BTreeSet<String>> = BTreeMap::new();
    // Removed scope that could not be fully observed: absence there proves nothing.
    let mut removals_visible = true;
    if request.leave {
        removals_visible = snapshots
            .iter()
            .all(|s| roles_observable(input, &s.provider));
        let assigned = profiles_of(&request.removed);
        let justify = |because: Vec<String>| {
            if because.is_empty() {
                Justification::Unexplained
            } else {
                Justification::Profiles(sorted(because))
            }
        };
        for path in &access {
            let because = assigned
                .iter()
                .filter(|(entry, _)| explains(path, entry))
                .flat_map(|(_, names)| names.iter().cloned())
                .collect();
            steps.add(match path.memberships.first() {
                None => remove_role(path, justify(because)),
                Some(m) => remove_membership(
                    &path.account,
                    &m.group,
                    vec![path.grant.id.clone()],
                    justify(because),
                ),
            });
        }
        for account in &accounts {
            // Memberships that grant nothing observed are still access to remove.
            for group in graph.direct(&account.key) {
                let reach = graph.reachable_from(&Subject::Group(group.clone()))?;
                let because = assigned
                    .iter()
                    .filter(|(entry, _)| match entry {
                        ExpectedEntry::Group { instance, group: g } => {
                            let key = EntityKey::new(instance, g);
                            key == group || reach.contains(&key)
                        }
                        ExpectedEntry::Role { .. } => false,
                    })
                    .flat_map(|(_, names)| names.iter().cloned())
                    .collect();
                steps.add(remove_membership(
                    &account.key,
                    &group,
                    vec![],
                    justify(because),
                ));
            }
            let (state, reason) = state_for(
                &account.key.provider,
                observable(input, &account.key.provider, "accounts"),
            );
            steps.add(PlannedStep {
                operation: Operation::ReviewAccountLifecycle {
                    instance: account.key.provider.clone(),
                    account: account.key.id.clone(),
                },
                state,
                reason: if state == EntryState::Finding {
                    "manual_review"
                } else {
                    reason
                },
                justification: justify(request.removed.iter().cloned().collect()),
                evidence: vec![],
                needs_account: None,
            });
        }
    } else {
        let after_groups: BTreeMap<EntityKey, &Vec<String>> = after
            .iter()
            .filter_map(|(entry, names)| match entry {
                ExpectedEntry::Group { instance, group } => {
                    Some((EntityKey::new(instance, group), names))
                }
                ExpectedEntry::Role { .. } => None,
            })
            .collect();
        // Remaining profiles that need the account's membership in `group`: a profile
        // group at or above it, or a profile role reached through it.
        let justifying = |account: &EntityKey, group: &EntityKey| {
            let reach = graph.reachable_from(&Subject::Group(group.clone()))?;
            let mut because = Vec::new();
            for (key, names) in &after_groups {
                if key == group || reach.contains(key) {
                    because.extend(names.iter().cloned());
                }
            }
            for path in access.iter().filter(|p| {
                p.account == *account && p.memberships.first().is_some_and(|m| m.group == *group)
            }) {
                for (entry, names) in &after {
                    if matches!(entry, ExpectedEntry::Role { .. }) && explains(path, entry) {
                        because.extend(names.iter().cloned());
                    }
                }
            }
            Ok::<_, DomainError>(sorted(because))
        };
        for (entry, names) in profiles_of(&request.removed) {
            if let Some(because) = after.get(entry) {
                kept.entry(entry.clone())
                    .or_default()
                    .extend(because.iter().cloned());
                continue;
            }
            let justification = Justification::Profiles(names);
            // (account, first membership hop, grant IDs of paths through it)
            let mut hops: Vec<(&EntityKey, EntityKey, Vec<String>)> = Vec::new();
            match entry {
                ExpectedEntry::Group { instance, group } => {
                    removals_visible &= observable(input, instance, "memberships");
                    let target = EntityKey::new(instance, group);
                    for account in accounts.iter().filter(|a| a.key.provider == *instance) {
                        for direct in graph.direct(&account.key) {
                            if direct != target
                                && !graph
                                    .reachable_from(&Subject::Group(direct.clone()))?
                                    .contains(&target)
                            {
                                continue;
                            }
                            let evidence = access
                                .iter()
                                .filter(|p| {
                                    p.account == account.key
                                        && p.memberships.first().is_some_and(|m| m.group == direct)
                                        && p.groups.iter().any(|g| g.key == target)
                                })
                                .map(|p| p.grant.id.clone())
                                .collect();
                            hops.push((&account.key, direct, evidence));
                        }
                    }
                }
                ExpectedEntry::Role { instance, .. } => {
                    removals_visible &= roles_observable(input, instance);
                    for path in access.iter().filter(|p| explains(p, entry)) {
                        match path.memberships.first() {
                            None => steps.add(remove_role(path, justification.clone())),
                            Some(m) => hops.push((
                                &path.account,
                                m.group.clone(),
                                vec![path.grant.id.clone()],
                            )),
                        }
                    }
                }
            }
            for (account, group, evidence) in hops {
                let because = justifying(account, &group)?;
                if because.is_empty() {
                    steps.add(remove_membership(
                        account,
                        &group,
                        evidence,
                        justification.clone(),
                    ));
                } else {
                    kept.entry(entry.clone()).or_default().extend(because);
                }
            }
        }
    }
    result.steps = steps.0.into_values().collect();
    result.kept = kept
        .into_iter()
        .map(|(entry, because)| KeptAccess {
            entry,
            because: because.into_iter().collect(),
        })
        .collect();
    // Removing access from an identity no provider knows proves nothing: a typo
    // would otherwise read as "nothing to remove".
    let unknown_removal =
        result.state == ComparisonState::NotFound && (request.leave || !request.removed.is_empty());
    result.complete = claimed_in.is_empty()
        && !unknown_removal
        && removals_visible
        && result.steps.iter().all(|s| s.state == EntryState::Finding);
    Ok(result)
}

/// What one canonical identity holds now, for verifying a plan's steps.
#[derive(Clone, Debug, Serialize)]
pub struct AttributedAccess {
    pub accounts: Vec<EntityKey>,
    pub paths: Vec<AccessPath>,
    pub member_of: BTreeSet<EntityKey>,
    /// Instances where an account this identity may own is not attributed to it.
    pub claimed_instances: BTreeSet<String>,
    /// Conflicting identity evidence: nothing can be attributed.
    pub ambiguous: bool,
}
pub fn attributed_access(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    identity: &str,
) -> Result<AttributedAccess, DomainError> {
    let index = IdentityIndex::build(snapshots, aliases)?;
    let (owned, claimed, _) = owned_accounts(&index);
    let ambiguous = index.conflicting.contains(identity);
    let accounts = if ambiguous {
        vec![]
    } else {
        owned.get(identity).cloned().unwrap_or_default()
    };
    let mut paths = paths(snapshots, accounts.iter().copied(), false)?;
    sort_paths(&mut paths);
    let graph = MembershipGraph::new(snapshots);
    let mut member_of = BTreeSet::new();
    for account in &accounts {
        member_of.extend(graph.reachable(&account.key)?);
    }
    Ok(AttributedAccess {
        accounts: accounts.iter().map(|a| a.key.clone()).collect(),
        paths,
        member_of,
        claimed_instances: claimed
            .get(identity)
            .into_iter()
            .flatten()
            .map(|i| (*i).to_owned())
            .collect(),
        ambiguous,
    })
}
