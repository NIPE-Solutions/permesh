// SPDX-License-Identifier: MIT
//! Verify a plan against a later capture. An add step is done once observed; a
//! removal is done only when absent from a complete capture of its instance.
//! Missing observation never proves absence.
use super::model::{AccessPlan, Kind, hash, plan_sha256};
use crate::{
    artifact::{Artifact, State},
    error::AppError,
};
use permesh_core::{self as core, Operation};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const CANNOT: &str = "cannot_verify";

pub(crate) fn verify(
    plan: &AccessPlan,
    artifact: &Artifact,
    input: Option<&core::ProfileInput>,
) -> Result<Value, AppError> {
    plan.validate()?;
    let (snapshots, aliases) = crate::resource_command::domain(artifact);
    let now = core::attributed_access(&snapshots, &aliases, &plan.identity)?;
    let observable: BTreeMap<&str, BTreeSet<&str>> = artifact
        .providers
        .iter()
        .filter(|p| p.state == State::Complete)
        .map(|p| {
            (
                p.instance.as_str(),
                p.capabilities.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    let can = |instance: &str, capability: &str| {
        observable
            .get(instance)
            .is_some_and(|caps| caps.contains(capability))
    };
    let roles_visible = |instance: &str| {
        can(instance, "grants") && (can(instance, "memberships") || !can(instance, "groups"))
    };
    let data: BTreeMap<&str, &core::Snapshot> =
        snapshots.iter().map(|s| (s.provider.as_str(), s)).collect();
    let mut checks = Vec::new();
    for step in &plan.steps {
        let instance = step.operation.instance();
        let has_account = now.accounts.iter().any(|a| a.provider == instance);
        // Presence is evidence in any capture; absence only in a complete one.
        let added = |observed: bool, visible: bool| {
            if observed {
                ("done", "observed")
            } else if !visible {
                (CANNOT, "instance_not_observable")
            } else if !has_account {
                (CANNOT, "no_mapped_account")
            } else {
                ("not_done", "not_observed")
            }
        };
        let removed = |present: bool, visible: bool| {
            if !visible {
                (CANNOT, "instance_not_observable")
            } else if present {
                ("not_done", "still_observed")
            } else {
                ("done", "no_longer_observed")
            }
        };
        let (state, reason) = match &step.operation {
            Operation::ReviewAccountLifecycle { .. }
            | Operation::ReviewOwnership { .. }
            | Operation::ReviewMachineDependency { .. } => (CANNOT, "manual_review"),
            _ if now.ambiguous || now.claimed_instances.contains(instance) => {
                (CANNOT, "ambiguous_account")
            }
            Operation::AccountNeeded { .. } => {
                if has_account {
                    ("done", "observed")
                } else if !can(instance, "accounts") {
                    (CANNOT, "instance_not_observable")
                } else {
                    (CANNOT, "no_mapped_account")
                }
            }
            Operation::AddMembership { group, .. } => added(
                now.member_of
                    .contains(&core::EntityKey::new(instance, group)),
                can(instance, "memberships"),
            ),
            Operation::AddRole { role, resource, .. } => added(
                now.paths.iter().any(|p| {
                    p.resource.key.provider == instance
                        && p.resource.key.id == *resource
                        && p.grant.role == *role
                }),
                roles_visible(instance),
            ),
            Operation::RemoveMembership { account, group, .. } => {
                let member = core::Subject::Account(core::EntityKey::new(instance, account));
                let group = core::EntityKey::new(instance, group);
                removed(
                    data.get(instance).is_some_and(|s| {
                        s.memberships
                            .iter()
                            .any(|m| m.member == member && m.group == group)
                    }),
                    can(instance, "memberships"),
                )
            }
            Operation::RemoveRole { grant_id, .. } => removed(
                data.get(instance)
                    .is_some_and(|s| s.grants.iter().any(|g| g.id == *grant_id)),
                roles_visible(instance),
            ),
        };
        checks.push(json!({
            "step": step.id,
            "operation": step.operation,
            "state": state,
            "reason": reason,
        }));
    }
    let mut gaps = Vec::new();
    if hash(&artifact.identity)? != plan.identity_context_sha256 {
        gaps.push("identity_context_changed");
    }
    if !plan.complete {
        gaps.push("plan_incomplete");
    }
    let mut extra = Vec::new();
    if plan.kind != Kind::Leave {
        match input {
            None => gaps.push("profiles_not_configured"),
            Some(input)
                if plan
                    .profiles_after
                    .iter()
                    .any(|p| !input.profiles.contains_key(p)) =>
            {
                gaps.push("profile_no_longer_defined")
            }
            Some(input) => extra = extra_access(plan, input, &snapshots, &aliases)?,
        }
    }
    let count = |state: &str| checks.iter().filter(|c| c["state"] == state).count();
    let manual = checks
        .iter()
        .filter(|c| c["reason"] == "manual_review")
        .count();
    let complete = artifact.complete()
        && gaps.is_empty()
        && count(CANNOT) == manual
        && extra.iter().all(|e| e["state"] == "finding");
    Ok(json!({
        "access_verification_version": 1,
        "plan_sha256": plan_sha256(plan)?,
        "identity": plan.identity,
        "kind": plan.kind,
        "complete": complete,
        "checks": checks,
        "extra": extra,
        "gaps": gaps,
        "counts": {
            "done": count("done"),
            "not_done": count("not_done"),
            "cannot_verify": count(CANNOT),
            "manual_review": manual,
            "extra": extra.len(),
        },
    }))
}

/// Access unexplained by the profiles the person holds after the change, as in
/// `profile check`.
fn extra_access(
    plan: &AccessPlan,
    input: &core::ProfileInput,
    snapshots: &[core::Snapshot],
    aliases: &core::Aliases,
) -> Result<Vec<Value>, AppError> {
    let mut input = input.clone();
    input.people = [(
        plan.identity.clone(),
        plan.profiles_after.iter().cloned().collect(),
    )]
    .into();
    input.directory_groups.clear();
    let comparison = core::compare_profiles(snapshots, aliases, &input)?;
    Ok(comparison
        .identities
        .iter()
        .filter(|i| i.identity == plan.identity)
        .flat_map(|i| &i.unexplained)
        .map(|row| {
            let path = &row.path;
            json!({
                "state": match row.state {
                    core::EntryState::Finding => "finding",
                    core::EntryState::CannotTell => "cannot_tell",
                },
                "instance": path.account.provider,
                "account": path.account.id,
                "resource": path.resource.key.id,
                "resource_name": path.resource.name,
                "role": path.grant.role,
                "grant_id": path.grant.id,
                "via": path.groups.iter().map(|g| g.key.id.as_str()).collect::<Vec<_>>(),
                "certainty": path.certainty(),
            })
        })
        .collect())
}
