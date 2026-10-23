// SPDX-License-Identifier: MIT
//! A change in someone's profiles (join, move, leave) as operation-shaped steps.
//! Pure planning over observations: every step is carried out by a person, and a
//! step that could not be fully observed stays `cannot_tell`.
use crate::identity::IdentityIndex;
use crate::profiles::{observable, owned_accounts, roles_observable};
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
    result.steps = steps.0.into_values().collect();
    result.complete =
        claimed_in.is_empty() && result.steps.iter().all(|s| s.state == EntryState::Finding);
    Ok(result)
}
