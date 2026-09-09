// SPDX-License-Identifier: MIT
//! Versioned advisory documents, independent of provider wire and domain serialization.
use crate::{
    artifact::{self, ProviderCapture, records::*},
    error::AppError,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    FreshDiscovery,
    SnapshotReplay,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annotations {
    pub version: u32,
    pub owners: Vec<OwnerAssertion>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerAssertion {
    pub account: EntityKey,
    pub identity: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ownership {
    pub account_observed_at: String,
    pub account: Account,
    pub identity: String,
    pub basis: OwnershipBasis,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipBasis {
    OrganizationalAssertion,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathEvidence {
    pub id: String,
    pub account: EntityKey,
    pub groups: Vec<Group>,
    pub memberships: Vec<Membership>,
    pub resource: Resource,
    pub grant: Grant,
    pub certainty: Certainty,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub capture: ProviderCapture,
    pub observation_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gap {
    pub provider: Option<String>,
    pub reason: GapReason,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapReason {
    CollectionIncomplete,
    ContextChanged,
    CaptureNotOrdered,
    PlanExpired,
    FutureCapture,
    IdentityNotEstablished,
    IdentityContextChanged,
    AdditionalAccountsObserved,
    OwnershipRequiresManualReview,
    UnsupportedCheck,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationKind {
    #[serde(rename = "review_direct_assignment")]
    DirectAssignment,
    #[serde(rename = "review_group_membership")]
    GroupMembership,
    #[serde(rename = "review_ownership")]
    Ownership,
    #[serde(rename = "review_machine_dependency")]
    MachineDependency,
    #[serde(rename = "review_account_lifecycle")]
    AccountLifecycle,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    ObserveAssignmentPath,
    ObserveMembership,
    ObserveAccountLifecycle,
    ManualOwnershipReview,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationReason {
    ObservedDirectAssignment,
    InheritedThroughMembership,
    OwnerRoleNeedsTransferReview,
    ExplicitMachineResponsibility,
    LifecycleDoesNotEstablishRevocation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recommendation {
    pub reason: RecommendationReason,
    pub id: String,
    pub kind: RecommendationKind,
    pub account: EntityKey,
    pub resource: Option<EntityKey>,
    pub evidence: Vec<String>,
    pub prerequisites: Vec<String>,
    pub verification: VerificationMethod,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipFinding {
    pub resource: EntityKey,
    pub observed_owner_accounts: usize,
    pub sole_observed_owner: bool,
    pub basis: OwnerFindingBasis,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerFindingBasis {
    CompleteVisibleGithubOrganizationMembership,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assessment {
    pub ownership_findings: Vec<OwnershipFinding>,
    pub target: Identity,
    pub authoritative_sources: Vec<String>,
    pub accounts: Vec<Account>,
    pub paths: Vec<PathEvidence>,
    pub memberships: Vec<Membership>,
    pub ownership: Vec<Ownership>,
    pub sources: Vec<Source>,
    pub gaps: Vec<Gap>,
    pub recommendations: Vec<Recommendation>,
    pub unsupported_checks: Vec<UnsupportedCheck>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedCheck {
    Invitations,
    Tokens,
    Sessions,
    SecretReadHistory,
    UniversalAccessDenial,
    SoleOwnerOutsideQualifiedScope,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub max_age_hours: u32,
    pub format: String,
    pub format_version: u32,
    pub created_at: String,
    pub expires_at: String,
    pub mode: Mode,
    pub source_snapshot_sha256: String,
    pub identity_context_sha256: String,
    pub assessment: Assessment,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    StillObserved,
    NoLongerObserved,
    AccountInactiveObserved,
    CannotVerify,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub evidence_id: String,
    pub provider: String,
    pub native_id: String,
    pub state: CheckState,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Assess,
    Plan,
    Verify,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentOrigin {
    CurrentCapture,
    PlanBaseline,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub verification_sources: Vec<Source>,
    pub assessment_origin: AssessmentOrigin,
    pub format: String,
    pub format_version: u32,
    pub operation: Operation,
    pub mode: Mode,
    pub generated_at: String,
    pub complete: bool,
    pub assessment: Assessment,
    pub checks: Vec<Check>,
    pub gaps: Vec<Gap>,
    pub plan_sha256: Option<String>,
    pub limitations: Vec<String>,
}
pub fn invalid() -> AppError {
    AppError::input(
        "Invalid offboarding document; check version, bounds, target, source context and evidence references",
    )
}
pub fn hash<T: Serialize>(value: &T) -> Result<String, AppError> {
    Ok(artifact::digest(
        &serde_json::to_vec(value).map_err(|_| invalid())?,
    ))
}
pub fn time(value: &str) -> Result<time::OffsetDateTime, AppError> {
    let value = time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
        .map_err(|_| invalid())?;
    if !value.offset().is_utc() {
        return Err(invalid());
    }
    Ok(value)
}
pub fn path_id(
    account: &EntityKey,
    grant: &Grant,
    memberships: &[Membership],
) -> Result<String, AppError> {
    hash(&(
        account,
        &grant.id,
        memberships
            .iter()
            .map(|m| (&m.member, &m.group))
            .collect::<Vec<_>>(),
    ))
}
pub fn evidence_start<'a>(
    sources: impl Iterator<Item = &'a ProviderCapture>,
) -> Result<time::OffsetDateTime, AppError> {
    let mut earliest = None;
    for p in sources {
        let mut start = time(&p.started_at)?;
        if let Some(artifact::SourceObservation::FileInventory { exported_at, .. }) =
            &p.source_observation
        {
            start = start.min(time(exported_at)?);
        }
        earliest = Some(earliest.map_or(start, |old: time::OffsetDateTime| old.min(start)));
    }
    earliest.ok_or_else(invalid)
}
