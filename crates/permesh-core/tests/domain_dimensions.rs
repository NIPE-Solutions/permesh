// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_core::*;

fn resource(id: &str, parent: Option<&str>) -> Resource {
    Resource {
        key: EntityKey::new("app", id),
        name: id.into(),
        kind: Some("app.folder".into()),
        parent: parent.map(|id| EntityKey::new("app", id)),
    }
}
fn fixture(kind: IdentityKind, affiliation: Affiliation, status: IdentityStatus) -> Snapshot {
    let mut s = Snapshot::new("app");
    s.identities.push(Identity {
        id: "owner".into(),
        kind,
        affiliation,
        status,
        verified_emails: vec!["a@example.com".into()],
    });
    s.accounts.push(Account {
        key: EntityKey::new("app", "a"),
        login: "a".into(),
        kind,
        affiliation,
        status: IdentityStatus::Active,
        verified_emails: vec!["a@example.com".into()],
    });
    s.resources = vec![resource("root", None), resource("child", Some("root"))];
    s.grants.push(Grant {
        id: "grant".into(),
        subject: Subject::Account(s.accounts[0].key.clone()),
        resource: s.resources[0].key.clone(),
        role: "read".into(),
        privilege: Privilege::Standard,
        evidence_kind: EvidenceKind::Assignment,
        certainty: Certainty::Observed,
        provenance: Provenance {
            method: "test".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
        },
    });
    s
}
#[test]
fn lifecycle_precedes_kind_and_affiliation_and_account_lifecycle_stays_independent() {
    for kind in [
        IdentityKind::Human,
        IdentityKind::Service,
        IdentityKind::Bot,
        IdentityKind::Unknown,
    ] {
        for affiliation in [
            Affiliation::Internal,
            Affiliation::External,
            Affiliation::Unknown,
        ] {
            for (status, reason) in [
                (IdentityStatus::Inactive, OrphanReason::InactiveIdentity),
                (IdentityStatus::Suspended, OrphanReason::SuspendedIdentity),
            ] {
                let mut s = fixture(kind, affiliation, status);
                let result =
                    query_orphaned(&[s.clone()], &Aliases::new(), &["app".into()]).unwrap();
                assert_eq!(result.accounts[0].reason, reason);
                assert_eq!(result.accounts[0].account.status, IdentityStatus::Active);
                s.complete = false;
                assert_eq!(
                    query_orphaned(&[s], &Aliases::new(), &["app".into()])
                        .unwrap()
                        .accounts[0]
                        .reason,
                    OrphanReason::Unassessed
                );
            }
        }
    }
    let mut s = fixture(
        IdentityKind::Human,
        Affiliation::Internal,
        IdentityStatus::Active,
    );
    s.accounts[0].status = IdentityStatus::Suspended;
    assert!(
        query_orphaned(&[s], &Aliases::new(), &["app".into()])
            .unwrap()
            .accounts
            .is_empty()
    );
}
#[test]
fn affiliation_conflicts_remain_ambiguous() {
    let s = fixture(
        IdentityKind::Human,
        Affiliation::Internal,
        IdentityStatus::Active,
    );
    let mut other = Snapshot::new("other");
    other.identities = s.identities.clone();
    other.identities[0].affiliation = Affiliation::External;
    assert!(matches!(
        query_user(&[s.clone(), other.clone()], &Aliases::new(), "owner"),
        Err(DomainError::Ambiguous)
    ));
    let result = query_orphaned(
        &[s, other],
        &Aliases::new(),
        &["app".into(), "other".into()],
    )
    .unwrap();
    assert_eq!(result.accounts[0].reason, OrphanReason::AmbiguousIdentity);
}
#[test]
fn containment_is_not_an_access_edge_and_membership_derives_only_observed_certainty() {
    let mut s = fixture(
        IdentityKind::Human,
        Affiliation::Internal,
        IdentityStatus::Active,
    );
    let direct = query_user(&[s.clone()], &Aliases::new(), "a").unwrap();
    assert_eq!(direct.access.len(), 1);
    assert_eq!(direct.access[0].resource.key.id, "root");
    assert_eq!(direct.access[0].certainty(), Certainty::Observed);
    s.groups.push(Group {
        key: EntityKey::new("app", "g"),
        name: "group".into(),
    });
    s.memberships.push(Membership {
        member: s.grants[0].subject.clone(),
        group: s.groups[0].key.clone(),
        provenance: s.grants[0].provenance.clone(),
    });
    s.grants[0].subject = Subject::Group(s.groups[0].key.clone());
    for evidence in [
        EvidenceKind::Permission,
        EvidenceKind::Assignment,
        EvidenceKind::PolicyAttachment,
        EvidenceKind::Unknown,
    ] {
        for certainty in [
            Certainty::Observed,
            Certainty::Derived,
            Certainty::Inferred,
            Certainty::Unknown,
        ] {
            s.grants[0].certainty = certainty;
            s.grants[0].evidence_kind = evidence;
            let result = query_user(&[s.clone()], &Aliases::new(), "a").unwrap();
            let path = &result.access[0];
            assert_eq!(
                path.certainty(),
                if certainty == Certainty::Observed {
                    Certainty::Derived
                } else {
                    certainty
                }
            );
            assert_eq!(path.grant.certainty, certainty);
            assert_eq!(path.grant.evidence_kind, evidence);
        }
    }
}
#[test]
fn validates_resource_kind_grammar_and_boundaries() {
    let mut s = Snapshot::new("app");
    s.resources.push(resource("r", None));
    for valid in [
        None,
        Some("a.b".into()),
        Some("google.workspace_org-unit".into()),
        Some(format!("{}.{}", "a".repeat(64), "b".repeat(63))),
    ] {
        s.resources[0].kind = valid;
        s.validate().unwrap();
    }
    for invalid in [
        "".into(),
        "single".into(),
        "A.b".into(),
        "a.2b".into(),
        "a..b".into(),
        "a.b.".into(),
        "a.b/c".into(),
        "a.é".into(),
        format!("{}.b", "a".repeat(65)),
        format!("{}.{}", "a".repeat(64), "b".repeat(64)),
    ] {
        s.resources[0].kind = Some(invalid);
        assert!(s.validate().is_err(), "accepted {:?}", s.resources[0].kind);
    }
}
#[test]
fn rejects_missing_cross_provider_self_and_cyclic_parents() {
    let mut s = Snapshot::new("app");
    s.resources = vec![resource("a", None), resource("b", Some("a"))];
    for parent in [
        EntityKey::new("app", "missing"),
        EntityKey::new("other", "a"),
        EntityKey::new("app", "a"),
        EntityKey::new("app", "b"),
    ] {
        s.resources[0].parent = Some(parent);
        assert!(s.validate().is_err());
    }
}
#[test]
fn validates_long_shuffled_parent_chains_without_recursion() {
    let mut s = Snapshot::new("app");
    s.resources = (0..50_000)
        .map(|i| {
            let parent = (i != 0).then(|| (i - 1).to_string());
            resource(&i.to_string(), parent.as_deref())
        })
        .collect();
    s.resources.reverse();
    s.validate().unwrap();
    s.resources.rotate_left(17_321);
    s.validate().unwrap();
    let root = s.resources.iter_mut().find(|r| r.key.id == "0").unwrap();
    root.parent = Some(EntityKey::new("app", "49999"));
    assert!(s.validate().is_err());
}

#[test]
fn large_parent_identifiers_are_charged_to_path_copy_budget() {
    let mut s = fixture(
        IdentityKind::Human,
        Affiliation::Internal,
        IdentityStatus::Active,
    );
    s.resources[0].key.id = "p".repeat(1024 * 1024);
    s.resources[1].parent = Some(s.resources[0].key.clone());
    s.grants[0].resource = s.resources[1].key.clone();
    let grant = s.grants[0].clone();
    s.grants = (0..128)
        .map(|i| Grant {
            id: i.to_string(),
            ..grant.clone()
        })
        .collect();
    s.validate().unwrap();
    assert!(matches!(
        query_user(&[s], &Aliases::new(), "a"),
        Err(DomainError::PathLimit)
    ));
}
