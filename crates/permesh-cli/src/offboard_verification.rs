// SPDX-License-Identifier: MIT
//! Fresh read-only comparisons preserve uncertainty and native observations.
use super::{
    assessment::{assess, recommendations},
    model::*,
    report,
};
use crate::{
    artifact::{self, Artifact, State, records::*},
    error::AppError,
};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn verify(plan: &Plan, current: &Artifact, mode: Mode) -> Result<Report, AppError> {
    plan.validate()?;
    current.validate()?;
    let annotations = Annotations {
        version: 1,
        owners: plan
            .assessment
            .ownership
            .iter()
            .map(|o| OwnerAssertion {
                account: o.account.key.clone(),
                identity: o.identity.clone(),
            })
            .collect(),
    };
    // Ownership references can legitimately disappear; manual review remains an unresolved check.
    let fresh = assess(current, &plan.assessment.target.id, None);
    let mut global = false;
    let assessment = match fresh {
        Ok(mut a) => {
            a.ownership = plan.assessment.ownership.clone();
            a.recommendations = recommendations(&a)?;
            a
        }
        Err(_) => {
            global = true;
            plan.assessment.clone()
        }
    };
    let mut result = report(Operation::Verify, mode, assessment)?;
    result
        .assessment
        .gaps
        .retain(|g| g.reason != GapReason::PlanExpired);
    result.gaps.retain(|g| g.reason != GapReason::PlanExpired);
    result.verification_sources = current
        .providers
        .iter()
        .map(|p| {
            let mut capture = p.clone();
            capture.data = None;
            Ok(Source {
                capture,
                observation_sha256: hash(p)?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    result.plan_sha256 = Some(hash(plan)?);
    if global {
        result.assessment_origin = AssessmentOrigin::PlanBaseline;
        result.gaps.push(Gap {
            provider: None,
            reason: GapReason::IdentityNotEstablished,
        });
    }
    if time(&current.completed_at)? > time(&result.generated_at)? {
        global = true;
        result.gaps.push(Gap {
            provider: None,
            reason: GapReason::FutureCapture,
        });
    }
    if hash(&current.identity)? != plan.identity_context_sha256 {
        global = true;
        result.gaps.push(Gap {
            provider: None,
            reason: GapReason::IdentityContextChanged,
        });
    }
    if time(&result.generated_at)? > time(&plan.expires_at)?
        || time(&current.completed_at)? > time(&plan.expires_at)?
        || time(&result.generated_at)? - evidence_start(current.providers.iter())?
            > time::Duration::hours(i64::from(plan.max_age_hours))
    {
        global = true;
        result.gaps.push(Gap {
            provider: None,
            reason: GapReason::PlanExpired,
        });
    }
    let old_accounts: BTreeSet<_> = plan
        .assessment
        .accounts
        .iter()
        .map(|a| (&a.key.provider, &a.key.id))
        .collect();
    if result
        .assessment
        .accounts
        .iter()
        .any(|a| !old_accounts.contains(&(&a.key.provider, &a.key.id)))
    {
        global = true;
        result.gaps.push(Gap {
            provider: None,
            reason: GapReason::AdditionalAccountsObserved,
        });
    }
    let correlated_accounts: BTreeSet<_> = result
        .assessment
        .accounts
        .iter()
        .map(|a| (&a.key.provider, &a.key.id))
        .collect();
    let mut comparable = BTreeMap::new();
    for source in &plan.assessment.sources {
        let old = &source.capture;
        let new = current
            .providers
            .iter()
            .find(|p| p.instance == old.instance);
        let reason = match new {
            None => Some(GapReason::CollectionIncomplete),
            Some(n) if old.state != State::Complete || n.state != State::Complete => {
                Some(GapReason::CollectionIncomplete)
            }
            Some(n) if !artifact::diff::comparable(old, n) => Some(GapReason::ContextChanged),
            Some(n) if !artifact::diff::ordered(old, n) => Some(GapReason::CaptureNotOrdered),
            // A target-filtered assessment can omit a still-present native account
            // when its correlation evidence changes. That is not account deletion.
            Some(n)
                if n.data.as_ref().is_some_and(|data| {
                    data.accounts.iter().any(|account| {
                        let key = (&account.key.provider, &account.key.id);
                        old_accounts.contains(&key) && !correlated_accounts.contains(&key)
                    })
                }) =>
            {
                Some(GapReason::AccountCorrelationChanged)
            }
            _ => None,
        };
        if let Some(reason) = reason {
            result.gaps.push(Gap {
                provider: Some(old.instance.clone()),
                reason,
            });
        }
        comparable.insert(old.instance.clone(), reason.is_none());
    }
    // Authority failures affect target identity, even when another access source succeeded.
    if plan
        .assessment
        .authoritative_sources
        .iter()
        .any(|s| !comparable.get(s).copied().unwrap_or(false))
    {
        global = true;
        result.gaps.push(Gap {
            provider: None,
            reason: GapReason::IdentityNotEstablished,
        });
    }
    let observed_paths: BTreeSet<_> = result
        .assessment
        .paths
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    let observed_accounts: BTreeMap<_, _> = result
        .assessment
        .accounts
        .iter()
        .map(|a| ((&a.key.provider, &a.key.id), a))
        .collect();
    let observed_memberships: BTreeSet<_> = result
        .assessment
        .memberships
        .iter()
        .map(|m| hash(&(&m.member, &m.group)))
        .collect::<Result<_, AppError>>()?;
    for path in &plan.assessment.paths {
        let observed = observed_paths.contains(path.id.as_str());
        let state = if global
            || !comparable
                .get(&path.account.provider)
                .copied()
                .unwrap_or(false)
        {
            CheckState::CannotVerify
        } else if observed {
            CheckState::StillObserved
        } else {
            CheckState::NoLongerObserved
        };
        result.checks.push(Check {
            evidence_id: path.id.clone(),
            provider: path.account.provider.clone(),
            native_id: path.grant.id.clone(),
            state,
        });
    }
    for old in &plan.assessment.memberships {
        let observed = observed_memberships.contains(&hash(&(&old.member, &old.group))?);
        let state = if global
            || !comparable
                .get(&old.group.provider)
                .copied()
                .unwrap_or(false)
        {
            CheckState::CannotVerify
        } else if observed {
            CheckState::StillObserved
        } else {
            CheckState::NoLongerObserved
        };
        result.checks.push(Check {
            evidence_id: hash(&(&old.member, &old.group))?,
            provider: old.group.provider.clone(),
            native_id: old.group.id.clone(),
            state,
        });
    }
    for old in &plan.assessment.accounts {
        let observed = observed_accounts.get(&(&old.key.provider, &old.key.id));
        let state = if global || !comparable.get(&old.key.provider).copied().unwrap_or(false) {
            CheckState::CannotVerify
        } else {
            match observed {
                Some(a)
                    if matches!(
                        a.status,
                        IdentityStatus::Inactive | IdentityStatus::Suspended
                    ) =>
                {
                    CheckState::AccountInactiveObserved
                }
                Some(_) => CheckState::StillObserved,
                None => CheckState::NoLongerObserved,
            }
        };
        result.checks.push(Check {
            evidence_id: hash(&old.key)?,
            provider: old.key.provider.clone(),
            native_id: old.key.id.clone(),
            state,
        });
    }
    for owner in annotations.owners {
        result.gaps.push(Gap {
            provider: Some(owner.account.provider),
            reason: GapReason::OwnershipRequiresManualReview,
        });
    }
    result.complete = result.gaps.is_empty();
    result.validate()?;
    Ok(result)
}
