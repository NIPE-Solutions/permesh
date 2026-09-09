// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_core::*;
use std::collections::BTreeSet;
fn fixture() -> (Snapshot, Aliases) {
    let mut s = Snapshot::new("p");
    s.complete = false;
    s.identities.push(Identity {
        id: "former".into(),
        kind: IdentityKind::Human,
        status: IdentityStatus::Inactive,
        affiliation: Affiliation::External,
        verified_emails: vec![],
    });
    s.accounts.push(Account {
        key: EntityKey::new("p", "1"),
        login: "service".into(),
        kind: IdentityKind::Service,
        status: IdentityStatus::Unknown,
        affiliation: Affiliation::External,
        verified_emails: vec![],
    });
    s.resources.push(Resource {
        key: EntityKey::new("p", "r"),
        name: "Resource".into(),
        kind: None,
        parent: None,
    });
    s.grants.push(Grant {
        id: "g".into(),
        subject: Subject::Account(EntityKey::new("p", "1")),
        resource: EntityKey::new("p", "r"),
        role: "Administrator".into(),
        privilege: Privilege::Admin,
        certainty: Certainty::Observed,
        evidence_kind: EvidenceKind::Assignment,
        provenance: Provenance {
            method: "fixture".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
        },
    });
    let aliases = Aliases::from([(
        "former".into(),
        std::collections::BTreeMap::from([("p".into(), vec!["1".into()])]),
    )]);
    (s, aliases)
}
#[test]
fn partial_observations_can_establish_findings_without_implying_clean_coverage() {
    let (s, a) = fixture();
    let checks = review_policy_access(&[s], &a, &["p".into()], &BTreeSet::new()).unwrap();
    assert_eq!(checks.len(), 3);
    assert!(checks.iter().all(|c| c.state == PolicyState::Finding));
}
#[test]
fn unknown_privilege_is_not_clean_and_explicit_owner_is_an_assertion() {
    let (mut s, a) = fixture();
    s.grants[0].privilege = Privilege::Unknown;
    let checks = review_policy_access(&[s.clone()], &a, &["p".into()], &BTreeSet::new()).unwrap();
    assert!(
        checks
            .iter()
            .filter(|c| c.rule != AccessRule::InactiveAccess)
            .all(|c| c.state == PolicyState::NotEvaluable)
    );
    s.grants[0].privilege = Privilege::Admin;
    let checks = review_policy_access(
        &[s],
        &a,
        &["p".into()],
        &BTreeSet::from([EntityKey::new("p", "1")]),
    )
    .unwrap();
    assert!(
        checks
            .iter()
            .any(|c| c.rule == AccessRule::MachineOwner && c.state == PolicyState::Pass)
    );
}
#[test]
fn untrusted_or_conflicting_identity_evidence_cannot_establish_inactivity() {
    let (mut s, a) = fixture();
    let checks = review_policy_access(&[s.clone()], &a, &[], &BTreeSet::new()).unwrap();
    assert!(
        checks
            .iter()
            .any(|c| c.rule == AccessRule::InactiveAccess && c.state == PolicyState::NotEvaluable)
    );
    let mut second = s.clone();
    second.provider = "other".into();
    second.accounts.clear();
    second.resources.clear();
    second.grants.clear();
    second.identities[0].status = IdentityStatus::Active;
    s.complete = true;
    let checks = review_policy_access(
        &[s, second],
        &a,
        &["p".into(), "other".into()],
        &BTreeSet::new(),
    )
    .unwrap();
    assert!(
        checks
            .iter()
            .any(|c| c.rule == AccessRule::InactiveAccess && c.state == PolicyState::NotEvaluable)
    );
}
