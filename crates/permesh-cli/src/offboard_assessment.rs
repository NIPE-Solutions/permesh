// SPDX-License-Identifier: MIT
//! Exact canonical target selection and scoped advisory evidence.
use super::model::*;
use crate::{
    artifact::{Artifact, State, records::*},
    error::AppError,
};
use std::collections::{BTreeMap, BTreeSet};
fn domain(a: &Artifact) -> Vec<permesh_core::Snapshot> {
    a.providers
        .iter()
        .filter_map(|p| p.data.as_ref().map(Into::into))
        .collect()
}
fn aliases(a: &Artifact) -> permesh_core::Aliases {
    let mut aliases = permesh_core::Aliases::new();
    for b in &a.identity.bindings {
        aliases
            .entry(b.identity.clone())
            .or_default()
            .entry(b.instance.clone())
            .or_default()
            .push(b.account.clone());
    }
    aliases
}
pub(super) fn assess(
    a: &Artifact,
    person: &str,
    annotations: Option<&Annotations>,
) -> Result<Assessment, AppError> {
    a.validate()?;
    let snapshots = domain(a);
    let aliases = aliases(a);
    let review =
        permesh_core::identity_review::review(&snapshots, &aliases, &a.identity.authorities)
            .map_err(|_| invalid())?;
    let target=review.identities.iter().find(|i|i.identity.id==person).filter(|i|!i.conflicting).ok_or_else(||AppError::input("Offboarding requires an exact unambiguous observed authoritative canonical identity"))?;
    if review.accounts.iter().any(|a|matches!(&a.resolution,permesh_core::IdentityResolution::Ambiguous{candidates} if candidates.iter().any(|id|id==person))){return Err(AppError::input("Offboarding target has conflicting account identity evidence"));}
    let access = permesh_core::query_user(&snapshots, &aliases, person)
        .map_err(|_| AppError::input("Cannot establish unambiguous target accounts and paths"))?;
    if access.identity.as_ref().is_none_or(|i| i.id != person) {
        return Err(invalid());
    }
    let accounts: Vec<Account> = access.accounts.iter().map(Into::into).collect();
    let paths = access
        .access
        .iter()
        .map(|p| {
            let account = (&p.account).into();
            let grant = (&p.grant).into();
            let memberships: Vec<Membership> = p.memberships.iter().map(Into::into).collect();
            Ok(PathEvidence {
                id: path_id(&account, &grant, &memberships)?,
                account,
                grant,
                memberships,
                groups: p.groups.iter().map(Into::into).collect(),
                resource: (&p.resource).into(),
                certainty: p.certainty().into(),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let account_keys: BTreeSet<_> = accounts
        .iter()
        .map(|a| (&a.key.provider, &a.key.id))
        .collect();
    let memberships=a.providers.iter().filter_map(|p|p.data.as_ref()).flat_map(|d|d.memberships.iter()).filter(|m|matches!(&m.member,Subject::Account(k) if account_keys.contains(&(&k.provider,&k.id)))).cloned().collect();
    let all_accounts: BTreeMap<_, _> = a
        .providers
        .iter()
        .filter_map(|p| p.data.as_ref())
        .flat_map(|d| &d.accounts)
        .map(|a| ((&a.key.provider, &a.key.id), a))
        .collect();
    let mut ownership = vec![];
    if let Some(annotations) = annotations {
        if annotations.version != 1 || annotations.owners.len() > 10000 {
            return Err(invalid());
        }
        let mut seen = BTreeSet::new();
        for o in &annotations.owners {
            if o.identity.is_empty() || !seen.insert((&o.account.provider, &o.account.id)) {
                return Err(invalid());
            }
            if o.identity != person {
                continue;
            }
            let machine = all_accounts
                .get(&(&o.account.provider, &o.account.id))
                .copied()
                .filter(|a| matches!(a.kind, IdentityKind::Service | IdentityKind::Bot))
                .ok_or_else(|| {
                    AppError::input(
                        "Owner annotations require an observed exact service or bot account",
                    )
                })?;
            ownership.push(Ownership {
                account_observed_at: a
                    .providers
                    .iter()
                    .find(|p| p.instance == o.account.provider)
                    .ok_or_else(invalid)?
                    .completed_at
                    .clone(),
                account: machine.clone(),
                identity: person.into(),
                basis: OwnershipBasis::OrganizationalAssertion,
            });
        }
    }
    let mut sources = vec![];
    let mut gaps = vec![];
    for p in &a.providers {
        let mut capture = p.clone();
        capture.data = None;
        sources.push(Source {
            capture,
            observation_sha256: hash(p)?,
        });
        if p.state != State::Complete {
            gaps.push(Gap {
                provider: Some(p.instance.clone()),
                reason: GapReason::CollectionIncomplete,
            });
        }
    }
    if time(&a.completed_at)? > time::OffsetDateTime::now_utc() {
        return Err(AppError::input("Capture time must not be in the future"));
    }
    if time::OffsetDateTime::now_utc() - evidence_start(a.providers.iter())?
        > time::Duration::hours(24)
    {
        gaps.push(Gap {
            provider: None,
            reason: GapReason::PlanExpired,
        });
    }
    let mut owner_index = BTreeMap::new();
    for source in a
        .providers
        .iter()
        .filter(|p| p.provider_type == "github" && p.state == State::Complete)
    {
        let Some(data) = &source.data else {
            continue;
        };
        let organizations: BTreeSet<_> = data
            .resources
            .iter()
            .filter(|r| r.kind.as_deref() == Some("github.organization"))
            .map(|r| (&r.key.provider, &r.key.id))
            .collect();
        for grant in data.grants.iter().filter(|g| {
            g.privilege == Privilege::Owner
                && organizations.contains(&(&g.resource.provider, &g.resource.id))
        }) {
            let entry = owner_index
                .entry((&grant.resource.provider, &grant.resource.id))
                .or_insert_with(|| (BTreeSet::new(), false));
            if grant.certainty != Certainty::Observed
                || !matches!(
                    grant.evidence_kind,
                    EvidenceKind::Assignment | EvidenceKind::Permission
                )
                || grant.provenance.method != "github.organization_role"
            {
                entry.1 = true;
            }
            match &grant.subject {
                Subject::Account(k) => {
                    entry.0.insert((&k.provider, &k.id));
                }
                _ => entry.1 = true,
            }
        }
    }
    let mut ownership_findings = vec![];
    let mut seen = BTreeSet::new();
    for path in &paths {
        let key = (&path.resource.key.provider, &path.resource.key.id);
        if path.grant.privilege != Privilege::Owner || !seen.insert(key) {
            continue;
        }
        let Some((owners, invalid)) = owner_index.get(&key) else {
            continue;
        };
        if *invalid || owners.is_empty() {
            continue;
        }
        ownership_findings.push(OwnershipFinding {
            resource: path.resource.key.clone(),
            observed_owner_accounts: owners.len(),
            sole_observed_owner: owners.len() == 1,
            basis: OwnerFindingBasis::CompleteVisibleGithubOrganizationMembership,
        });
    }
    let mut result = Assessment {
        ownership_findings,
        target: (&target.identity).into(),
        authoritative_sources: target.sources.clone(),
        accounts,
        paths,
        memberships,
        ownership,
        sources,
        gaps,
        recommendations: vec![],
        unsupported_checks: vec![
            UnsupportedCheck::Invitations,
            UnsupportedCheck::Tokens,
            UnsupportedCheck::Sessions,
            UnsupportedCheck::SecretReadHistory,
            UnsupportedCheck::UniversalAccessDenial,
            UnsupportedCheck::SoleOwnerOutsideQualifiedScope,
        ],
    };
    result.recommendations = recommendations(&result)?;
    result.validate()?;
    Ok(result)
}
fn recommendation(
    kind: RecommendationKind,
    account: &EntityKey,
    resource: Option<EntityKey>,
    evidence: Vec<String>,
    verification: VerificationMethod,
) -> Result<Recommendation, AppError> {
    Ok(Recommendation {
        reason: match kind {
            RecommendationKind::DirectAssignment => RecommendationReason::ObservedDirectAssignment,
            RecommendationKind::GroupMembership => RecommendationReason::InheritedThroughMembership,
            RecommendationKind::Ownership => RecommendationReason::OwnerRoleNeedsTransferReview,
            RecommendationKind::MachineDependency => {
                RecommendationReason::ExplicitMachineResponsibility
            }
            RecommendationKind::AccountLifecycle => {
                RecommendationReason::LifecycleDoesNotEstablishRevocation
            }
        },
        id: hash(&(kind, account, &resource, &evidence))?,
        kind,
        account: account.clone(),
        resource,
        evidence,
        prerequisites: vec![],
        verification,
    })
}
pub(super) fn recommendations(a: &Assessment) -> Result<Vec<Recommendation>, AppError> {
    let mut result = vec![];
    for owner in &a.ownership {
        result.push(recommendation(
            RecommendationKind::MachineDependency,
            &owner.account.key,
            None,
            vec![hash(&owner.account.key)?],
            VerificationMethod::ManualOwnershipReview,
        )?);
    }
    for p in &a.paths {
        if p.grant.privilege == Privilege::Owner {
            result.push(recommendation(
                RecommendationKind::Ownership,
                &p.account,
                Some(p.resource.key.clone()),
                vec![p.id.clone()],
                VerificationMethod::ManualOwnershipReview,
            )?);
        }
        result.push(recommendation(
            if p.memberships.is_empty() {
                RecommendationKind::DirectAssignment
            } else {
                RecommendationKind::GroupMembership
            },
            &p.account,
            Some(p.resource.key.clone()),
            vec![p.id.clone()],
            VerificationMethod::ObserveAssignmentPath,
        )?);
    }
    for m in &a.memberships {
        if let Subject::Account(k) = &m.member {
            result.push(recommendation(
                RecommendationKind::GroupMembership,
                k,
                Some(m.group.clone()),
                vec![hash(&(k, &m.group))?],
                VerificationMethod::ObserveMembership,
            )?);
        }
    }
    let prerequisites: Vec<_> = result
        .iter()
        .filter(|r| {
            matches!(
                r.kind,
                RecommendationKind::Ownership | RecommendationKind::MachineDependency
            )
        })
        .map(|r| r.id.clone())
        .collect();
    if prerequisites
        .len()
        .saturating_mul(result.len().saturating_add(a.accounts.len()))
        > 100000
    {
        return Err(AppError::input(
            "Offboarding recommendation dependencies exceed the review budget; narrow the target scope",
        ));
    }
    for r in &mut result {
        if !matches!(
            r.kind,
            RecommendationKind::Ownership | RecommendationKind::MachineDependency
        ) {
            r.prerequisites = prerequisites.clone();
        }
    }
    for account in &a.accounts {
        let mut r = recommendation(
            RecommendationKind::AccountLifecycle,
            &account.key,
            None,
            vec![hash(&account.key)?],
            VerificationMethod::ObserveAccountLifecycle,
        )?;
        r.prerequisites = prerequisites.clone();
        result.push(r);
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result.dedup_by(|a, b| a.id == b.id);
    Ok(result)
}
