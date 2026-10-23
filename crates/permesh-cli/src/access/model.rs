// SPDX-License-Identifier: MIT
//! `permesh_access_plan` v1: a versioned plan document bound to the evidence it was
//! built from. Step IDs are the digest of the operation, so the same operation has
//! the same ID in every run; the pull-request flow relies on that.
use crate::{
    artifact::{self, Artifact},
    error::AppError,
    offboard,
};
use permesh_core as core;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use time::format_description::well_known::Rfc3339;

pub const FORMAT: &str = "permesh_access_plan";
pub const MAX_STEPS: usize = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Join,
    Move,
    Leave,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    ObserveMembership,
    ObserveRole,
    ObserveAccount,
    ObserveAbsence,
    ManualReview,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub operation: core::Operation,
    /// `planned` or `cannot_tell`.
    pub state: String,
    pub reason: String,
    pub justification: Value,
    pub evidence: Vec<String>,
    pub prerequisites: Vec<String>,
    pub verification: Verification,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessPlan {
    pub format: String,
    pub format_version: u32,
    pub kind: Kind,
    pub identity: String,
    pub profiles_after: Vec<String>,
    pub profiles_removed: Vec<String>,
    pub created_at: String,
    pub expires_at: String,
    pub max_age_hours: u32,
    pub mode: offboard::Mode,
    pub source_snapshot_sha256: String,
    pub identity_context_sha256: String,
    pub profiles_sha256: String,
    pub complete: bool,
    pub steps: Vec<Step>,
    pub kept: Vec<Value>,
    /// Set on execution plans: the digest of the plan reviewed before merge.
    pub reviewed_plan_sha256: Option<String>,
}

pub fn invalid() -> AppError {
    AppError::input("Invalid access plan; check version, bounds, step IDs and references")
}
pub fn hash<T: Serialize>(value: &T) -> Result<String, AppError> {
    Ok(artifact::digest(
        &serde_json::to_vec(value).map_err(|_| invalid())?,
    ))
}
pub fn step_id(operation: &core::Operation) -> Result<String, AppError> {
    hash(operation)
}
pub fn plan_sha256(plan: &AccessPlan) -> Result<String, AppError> {
    hash(plan)
}
pub fn verification(operation: &core::Operation) -> Verification {
    use core::Operation::*;
    match operation {
        AddMembership { .. } => Verification::ObserveMembership,
        AddRole { .. } => Verification::ObserveRole,
        AccountNeeded { .. } => Verification::ObserveAccount,
        RemoveMembership { .. } | RemoveRole { .. } => Verification::ObserveAbsence,
        ReviewAccountLifecycle { .. } | ReviewOwnership { .. } | ReviewMachineDependency { .. } => {
            Verification::ManualReview
        }
    }
}
fn is_review_first(operation: &core::Operation) -> bool {
    matches!(
        operation,
        core::Operation::ReviewOwnership { .. } | core::Operation::ReviewMachineDependency { .. }
    )
}
fn time(value: &str) -> Result<time::OffsetDateTime, AppError> {
    offboard::time(value).map_err(|_| invalid())
}

#[allow(clippy::too_many_arguments)]
pub fn build(
    kind: Kind,
    result: &core::PlanResult,
    request: &core::PlanRequest,
    artifact: &Artifact,
    input: &core::ProfileInput,
    mode: offboard::Mode,
    hours: u32,
    now: time::OffsetDateTime,
) -> Result<AccessPlan, AppError> {
    if hours == 0 || hours > 168 {
        return Err(AppError::input(
            "Plan freshness must be 1 through 168 hours",
        ));
    }
    let expires = offboard::evidence_start(artifact.providers.iter())?
        + time::Duration::hours(i64::from(hours));
    if expires <= now {
        return Err(AppError::input(
            "Evidence is too old for this plan freshness policy; capture fresh observations",
        ));
    }
    let format = |t: time::OffsetDateTime| t.format(&Rfc3339).map_err(|_| invalid());
    // Ownership and machine dependencies are reviewed before any other change.
    let reviews = result
        .steps
        .iter()
        .filter(|s| is_review_first(&s.operation))
        .map(|s| step_id(&s.operation))
        .collect::<Result<Vec<_>, _>>()?;
    let mut steps = Vec::with_capacity(result.steps.len());
    for planned in &result.steps {
        let mut prerequisites = match &planned.needs_account {
            Some(instance) => vec![step_id(&core::Operation::AccountNeeded {
                instance: instance.clone(),
            })?],
            None => vec![],
        };
        if !is_review_first(&planned.operation) {
            prerequisites.extend(reviews.iter().cloned());
        }
        steps.push(Step {
            id: step_id(&planned.operation)?,
            operation: planned.operation.clone(),
            state: match planned.state {
                core::EntryState::Finding => "planned",
                core::EntryState::CannotTell => "cannot_tell",
            }
            .into(),
            reason: planned.reason.into(),
            justification: serde_json::to_value(&planned.justification).map_err(|_| invalid())?,
            evidence: planned.evidence.clone(),
            prerequisites,
            verification: verification(&planned.operation),
        });
    }
    let plan = AccessPlan {
        format: FORMAT.into(),
        format_version: 1,
        kind,
        identity: request.identity.clone(),
        profiles_after: request.after.iter().cloned().collect(),
        profiles_removed: request.removed.iter().cloned().collect(),
        created_at: format(now)?,
        expires_at: format(expires)?,
        max_age_hours: hours,
        mode,
        source_snapshot_sha256: hash(artifact)?,
        identity_context_sha256: hash(&artifact.identity)?,
        profiles_sha256: hash(&input.profiles)?,
        complete: result.complete && artifact.complete(),
        steps,
        kept: result
            .kept
            .iter()
            .map(|k| serde_json::to_value(k).map_err(|_| invalid()))
            .collect::<Result<_, _>>()?,
        reviewed_plan_sha256: None,
    };
    plan.validate()?;
    Ok(plan)
}

fn strictly_sorted(names: &[String]) -> bool {
    names.windows(2).all(|w| w[0] < w[1])
}

impl AccessPlan {
    /// Structural and binding checks for an imported plan; it stays data either way.
    pub fn validate(&self) -> Result<(), AppError> {
        let created = time(&self.created_at)?;
        let expires = time(&self.expires_at)?;
        if self.format != FORMAT
            || self.format_version != 1
            || self.identity.is_empty()
            || self.max_age_hours == 0
            || self.max_age_hours > 168
            || expires <= created
            || expires > created + time::Duration::hours(i64::from(self.max_age_hours))
            || !strictly_sorted(&self.profiles_after)
            || !strictly_sorted(&self.profiles_removed)
            || self.steps.len() > MAX_STEPS
            || self.kept.len() > MAX_STEPS
            || ![
                &self.source_snapshot_sha256,
                &self.identity_context_sha256,
                &self.profiles_sha256,
            ]
            .into_iter()
            .chain(&self.reviewed_plan_sha256)
            .all(|d| artifact::valid_digest(d))
        {
            return Err(invalid());
        }
        let mut ids = BTreeSet::new();
        for step in &self.steps {
            if step.id != step_id(&step.operation)?
                || !ids.insert(step.id.as_str())
                || !matches!(step.state.as_str(), "planned" | "cannot_tell")
                || step.verification != verification(&step.operation)
                || step.evidence.len() > MAX_STEPS
            {
                return Err(invalid());
            }
        }
        for step in &self.steps {
            if step
                .prerequisites
                .iter()
                .any(|p| p == &step.id || !ids.contains(p.as_str()))
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::artifact::{IdentityContext, ProviderCapture, State};
    use std::collections::BTreeSet;

    fn artifact(started_at: &str) -> Artifact {
        Artifact {
            format: "permesh_snapshot".into(),
            format_version: 1,
            producer_version: "test".into(),
            started_at: started_at.into(),
            completed_at: started_at.into(),
            identity: IdentityContext {
                authorities: vec!["demo".into()],
                bindings: vec![],
            },
            providers: vec![ProviderCapture {
                instance: "demo".into(),
                provider_type: "demo".into(),
                provider_version: None,
                executable_sha256: None,
                capabilities: vec!["memberships".into()],
                context_sha256: "0".repeat(64),
                configured_scope: Default::default(),
                started_at: started_at.into(),
                completed_at: started_at.into(),
                state: State::Complete,
                limitations: vec![],
                data: None,
                failure_code: None,
                source_observation: None,
            }],
        }
    }
    fn step(operation: core::Operation, needs_account: Option<&str>) -> core::PlannedStep {
        core::PlannedStep {
            operation,
            state: core::EntryState::Finding,
            reason: "no_mapped_account",
            justification: core::Justification::Profiles(vec!["backend".into()]),
            evidence: vec![],
            needs_account: needs_account.map(str::to_owned),
        }
    }
    fn result() -> core::PlanResult {
        core::PlanResult {
            identity: "carol@example.com".into(),
            state: core::ComparisonState::NotFound,
            accounts: vec![],
            steps: vec![
                step(
                    core::Operation::AccountNeeded {
                        instance: "demo".into(),
                    },
                    None,
                ),
                step(
                    core::Operation::AddMembership {
                        instance: "demo".into(),
                        group: "team-10".into(),
                    },
                    Some("demo"),
                ),
            ],
            kept: vec![],
            complete: true,
        }
    }
    fn request() -> core::PlanRequest {
        core::PlanRequest {
            identity: "carol@example.com".into(),
            after: BTreeSet::from(["backend".to_string()]),
            removed: BTreeSet::new(),
            leave: false,
        }
    }
    fn now() -> time::OffsetDateTime {
        time::OffsetDateTime::parse(
            "2026-10-20T10:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
    }
    fn plan(hours: u32) -> Result<AccessPlan, AppError> {
        build(
            Kind::Join,
            &result(),
            &request(),
            &artifact("2026-10-20T09:00:00Z"),
            &core::ProfileInput::default(),
            offboard::Mode::SnapshotReplay,
            hours,
            now(),
        )
    }

    #[test]
    fn plan_round_trips_and_validates() {
        let plan = plan(24).unwrap();
        plan.validate().unwrap();
        assert_eq!(plan.format, "permesh_access_plan");
        assert_eq!(plan.format_version, 1);
        assert_eq!(plan.expires_at, "2026-10-21T09:00:00Z");
        assert_eq!(plan.profiles_after, vec!["backend".to_string()]);
        let account = &plan.steps[0];
        let add = &plan.steps[1];
        assert_eq!(account.verification, Verification::ObserveAccount);
        assert_eq!(add.verification, Verification::ObserveMembership);
        assert_eq!(add.prerequisites, vec![account.id.clone()]);
        assert_eq!(account.id, step_id(&account.operation).unwrap());
        assert_eq!(add.state, "planned");
        assert_eq!(
            add.justification,
            serde_json::json!({"profiles": ["backend"]})
        );
        let text = serde_json::to_vec(&plan).unwrap();
        let back: AccessPlan = serde_json::from_slice(&text).unwrap();
        back.validate().unwrap();
        assert_eq!(plan_sha256(&back).unwrap(), plan_sha256(&plan).unwrap());
    }

    #[test]
    fn step_ids_are_stable_across_runs() {
        let a = plan(24).unwrap();
        let b = plan(48).unwrap();
        let ids = |p: &AccessPlan| p.steps.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&a), ids(&b));
    }

    #[test]
    fn tampered_plan_is_rejected() {
        let mut plan = plan(24).unwrap();
        plan.steps[1].operation = core::Operation::AddMembership {
            instance: "demo".into(),
            group: "team-99".into(),
        };
        let error = plan.validate().unwrap_err();
        assert_eq!(error.code, 2);
        assert!(
            error.message.contains("Invalid access plan"),
            "{}",
            error.message
        );
    }

    #[test]
    fn unknown_prerequisite_is_rejected() {
        let mut plan = plan(24).unwrap();
        plan.steps[1].prerequisites = vec!["f".repeat(64)];
        assert_eq!(plan.validate().unwrap_err().code, 2);
    }

    #[test]
    fn duplicate_step_is_rejected() {
        let mut plan = plan(24).unwrap();
        let copy = plan.steps[0].clone();
        plan.steps.push(copy);
        assert_eq!(plan.validate().unwrap_err().code, 2);
    }

    #[test]
    fn freshness_bounds_are_enforced() {
        let mut plan = plan(24).unwrap();
        for hours in [0, 169] {
            plan.max_age_hours = hours;
            assert_eq!(plan.validate().unwrap_err().code, 2, "{hours}");
        }
        let mut plan = super::tests::plan(24).unwrap();
        plan.expires_at = "2026-10-25T09:00:00Z".into();
        assert_eq!(plan.validate().unwrap_err().code, 2);
        // Evidence older than the window cannot produce a plan.
        let error = build(
            Kind::Join,
            &result(),
            &request(),
            &artifact("2026-10-19T09:00:00Z"),
            &core::ProfileInput::default(),
            offboard::Mode::SnapshotReplay,
            24,
            now(),
        )
        .unwrap_err();
        assert_eq!(error.code, 2);
        assert!(
            error.message.contains("Evidence is too old"),
            "{}",
            error.message
        );
    }
}
