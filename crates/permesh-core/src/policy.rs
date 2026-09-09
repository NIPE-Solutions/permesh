// SPDX-License-Identifier: MIT
//! Focused read-only access checks. Local ownership assertions are not provider facts.
use crate::{
    identity::IdentityIndex,
    traversal::{MAX_PATH_STEPS, paths, sort_paths},
    *,
};
use std::collections::BTreeSet;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessRule {
    InactiveAccess,
    ExternalPrivileged,
    MachineOwner,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyState {
    Pass,
    Finding,
    NotEvaluable,
}
#[derive(Clone, Debug)]
pub struct AccessCheck {
    pub rule: AccessRule,
    pub state: PolicyState,
    pub instance: String,
    pub account: Option<EntityKey>,
    pub resource: EntityKey,
    pub grant_id: String,
    pub reason: &'static str,
}
fn privileged(value: Privilege) -> Option<bool> {
    match value {
        Privilege::Standard => Some(false),
        Privilege::Unknown => None,
        _ => Some(true),
    }
}
fn state(violation: bool) -> PolicyState {
    if violation {
        PolicyState::Finding
    } else {
        PolicyState::Pass
    }
}
pub fn review_policy_access(
    snapshots: &[Snapshot],
    aliases: &Aliases,
    authorities: &[String],
    owners: &BTreeSet<EntityKey>,
) -> Result<Vec<AccessCheck>, DomainError> {
    let authorities = authorities.iter().map(String::as_str).collect();
    let index = IdentityIndex::build_with_authorities(snapshots, aliases, &authorities)?;
    let mut access = paths(snapshots, index.accounts.values().copied(), false)?;
    sort_paths(&mut access);
    let mut observed = BTreeSet::new();
    let mut unique = BTreeSet::new();
    let mut checks = Vec::new();
    let mut budget = crate::path_budget::CopyBudget::new();
    for path in access {
        observed.insert((path.resource.key.provider.clone(), path.grant.id.clone()));
        if !unique.insert((path.account.clone(), path.grant.id.clone())) {
            continue;
        }
        let account = index
            .accounts
            .get(&path.account)
            .ok_or(DomainError::Reference)?;
        let resolution = index.resolve(&path.account);
        let identity = if let IdentityResolution::Resolved { identity } = &resolution {
            Some(identity)
        } else {
            None
        };
        let ambiguous = matches!(resolution, IdentityResolution::Ambiguous { .. });
        let known_evidence = matches!(
            path.grant.certainty,
            Certainty::Observed | Certainty::Derived
        );
        let known_assignment = known_evidence
            && matches!(
                path.grant.evidence_kind,
                EvidenceKind::Permission | EvidenceKind::Assignment
            );
        let rank = privileged(path.grant.privilege);
        let inactive = identity.map(|i| i.status);
        let affiliation = identity
            .map(|i| i.affiliation)
            .filter(|a| *a != Affiliation::Unknown)
            .unwrap_or(account.affiliation);
        let machine = matches!(account.kind, IdentityKind::Service | IdentityKind::Bot)
            || identity
                .is_some_and(|i| matches!(i.kind, IdentityKind::Service | IdentityKind::Bot));
        let human = account.kind == IdentityKind::Human
            || identity.is_some_and(|i| i.kind == IdentityKind::Human);
        let inactive_state = if !known_evidence {
            PolicyState::NotEvaluable
        } else {
            match inactive {
                Some(IdentityStatus::Inactive) => PolicyState::Finding,
                Some(IdentityStatus::Active | IdentityStatus::Suspended) => PolicyState::Pass,
                _ => PolicyState::NotEvaluable,
            }
        };
        let external_state = if ambiguous || !known_assignment {
            PolicyState::NotEvaluable
        } else {
            match (affiliation, rank) {
                (_, Some(false)) | (Affiliation::Internal, _) => PolicyState::Pass,
                (Affiliation::External, Some(true)) => PolicyState::Finding,
                _ => PolicyState::NotEvaluable,
            }
        };
        let owner_state = if ambiguous || !known_assignment {
            PolicyState::NotEvaluable
        } else {
            match (machine, human, rank) {
                (_, _, Some(false)) | (false, true, _) => PolicyState::Pass,
                (true, _, Some(true)) => state(!owners.contains(&path.account)),
                _ => PolicyState::NotEvaluable,
            }
        };
        for (rule, state, reason) in [
            (
                AccessRule::InactiveAccess,
                inactive_state,
                "authoritative_lifecycle_and_access",
            ),
            (
                AccessRule::ExternalPrivileged,
                external_state,
                "external_classification_and_privileged_assignment",
            ),
            (
                AccessRule::MachineOwner,
                owner_state,
                "privileged_machine_and_local_owner_assertion",
            ),
        ] {
            if checks.len() >= MAX_PATH_STEPS {
                return Err(DomainError::PathLimit);
            }
            budget.charge(
                size_of::<AccessCheck>()
                    .saturating_add(path.account.provider.len())
                    .saturating_add(crate::path_budget::key_heap(&path.account))
                    .saturating_add(crate::path_budget::key_heap(&path.resource.key))
                    .saturating_add(path.grant.id.len()),
            )?;
            checks.push(AccessCheck {
                rule,
                state,
                instance: path.account.provider.clone(),
                account: Some(path.account.clone()),
                resource: path.resource.key.clone(),
                grant_id: path.grant.id.clone(),
                reason,
            });
        }
    }
    for snapshot in snapshots {
        for grant in &snapshot.grants {
            if matches!(grant.subject, Subject::Group(_))
                && (!snapshot.complete
                    || !observed.contains(&(snapshot.provider.clone(), grant.id.clone())))
            {
                for rule in [
                    AccessRule::InactiveAccess,
                    AccessRule::ExternalPrivileged,
                    AccessRule::MachineOwner,
                ] {
                    if checks.len() >= MAX_PATH_STEPS {
                        return Err(DomainError::PathLimit);
                    }
                    budget.charge(
                        size_of::<AccessCheck>()
                            .saturating_add(snapshot.provider.len())
                            .saturating_add(crate::path_budget::key_heap(&grant.resource))
                            .saturating_add(grant.id.len()),
                    )?;
                    checks.push(AccessCheck {
                        rule,
                        state: PolicyState::NotEvaluable,
                        instance: snapshot.provider.clone(),
                        account: None,
                        resource: grant.resource.clone(),
                        grant_id: grant.id.clone(),
                        reason: "group_membership_not_fully_resolved",
                    });
                }
            }
        }
    }
    checks.sort_by(|a, b| {
        (&a.rule, &a.instance, &a.account, &a.resource, &a.grant_id).cmp(&(
            &b.rule,
            &b.instance,
            &b.account,
            &b.resource,
            &b.grant_id,
        ))
    });
    Ok(checks)
}
