// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_core::*;
fn fixture() -> Snapshot {
    let mut s = Snapshot::new("p");
    let key = |id| EntityKey::new("p", id);
    s.accounts.push(Account {
        key: key("a"),
        login: "unmapped".into(),
        kind: IdentityKind::Unknown,
        status: IdentityStatus::Unknown,
        affiliation: Affiliation::Unknown,
        verified_emails: vec![],
    });
    s.resources.push(Resource {
        key: key("r"),
        name: "shared".into(),
        kind: None,
        parent: None,
    });
    s.groups = vec![
        Group {
            key: key("child"),
            name: "Child".into(),
        },
        Group {
            key: key("parent"),
            name: "Parent".into(),
        },
        Group {
            key: key("unresolved"),
            name: "No visible members".into(),
        },
    ];
    let provenance = || Provenance {
        method: "fixture".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
    };
    s.memberships = vec![
        Membership {
            member: Subject::Account(key("a")),
            group: key("child"),
            provenance: provenance(),
        },
        Membership {
            member: Subject::Group(key("child")),
            group: key("parent"),
            provenance: provenance(),
        },
    ];
    for (id, subject, certainty) in [
        ("direct", Subject::Account(key("a")), Certainty::Observed),
        ("nested", Subject::Group(key("parent")), Certainty::Observed),
        ("effective", Subject::Account(key("a")), Certainty::Derived),
        (
            "unresolved",
            Subject::Group(key("unresolved")),
            Certainty::Observed,
        ),
    ] {
        s.grants.push(Grant {
            id: id.into(),
            subject,
            resource: key("r"),
            role: "access".into(),
            privilege: Privilege::Standard,
            certainty,
            evidence_kind: EvidenceKind::Assignment,
            provenance: provenance(),
        });
    }
    s
}
#[test]
fn resource_keeps_unmapped_nested_and_unresolved_evidence_without_relabeling_effective_grants() {
    let s = fixture();
    let result = query_resource(&[s], &Aliases::new(), &[], &EntityKey::new("p", "r")).unwrap();
    assert_eq!(result.accounts.len(), 1);
    assert!(matches!(
        result.accounts[0].identity,
        IdentityResolution::Unmapped
    ));
    assert_eq!(result.access.len(), 3);
    assert_eq!(result.grants.len(), 4);
    assert_eq!(result.unresolved_grants.len(), 1);
    let nested = result
        .access
        .iter()
        .find(|p| p.grant.id == "nested")
        .unwrap();
    assert_eq!(nested.groups.len(), 2);
    assert_eq!(nested.certainty(), Certainty::Derived);
    let effective = result
        .access
        .iter()
        .find(|p| p.grant.id == "effective")
        .unwrap();
    assert_eq!(effective.grant.certainty, Certainty::Derived);
}
#[test]
fn resource_counts_unique_grants_separately_and_is_stable_under_input_reordering() {
    let mut s = fixture();
    s.memberships.push(Membership {
        member: Subject::Account(EntityKey::new("p", "a")),
        group: EntityKey::new("p", "parent"),
        provenance: Provenance {
            method: "second route".into(),
            observed_at: "2026-01-01T00:00:00Z".into(),
        },
    });
    let first = query_resource(
        &[s.clone()],
        &Aliases::new(),
        &[],
        &EntityKey::new("p", "r"),
    )
    .unwrap();
    s.memberships.reverse();
    s.grants.reverse();
    s.groups.reverse();
    let second = query_resource(&[s], &Aliases::new(), &[], &EntityKey::new("p", "r")).unwrap();
    assert_eq!(first.access.len(), 4);
    assert_eq!(first.grants.len(), 4);
    assert_eq!(format!("{:?}", first), format!("{:?}", second));
}

#[test]
fn unresolved_resource_copy_amplification_has_a_byte_budget() {
    let mut s = fixture();
    s.accounts.clear();
    s.memberships.clear();
    s.grants.clear();
    s.resources[0].name = "x".repeat(1024 * 1024);
    for n in 0..100 {
        s.grants.push(Grant {
            id: format!("g-{n}"),
            subject: Subject::Group(EntityKey::new("p", "unresolved")),
            resource: EntityKey::new("p", "r"),
            role: "Reader".into(),
            privilege: Privilege::Standard,
            certainty: Certainty::Observed,
            evidence_kind: EvidenceKind::Assignment,
            provenance: Provenance {
                method: "fixture".into(),
                observed_at: "2026-01-01T00:00:00Z".into(),
            },
        });
    }
    assert!(matches!(
        query_resource(&[s], &Aliases::new(), &[], &EntityKey::new("p", "r")),
        Err(DomainError::PathLimit)
    ));
}

#[test]
fn resource_selection_does_not_traverse_unrelated_deep_grant_branches() {
    let mut s = fixture();
    let provenance = || Provenance {
        method: "fixture".into(),
        observed_at: "2026-01-01T00:00:00Z".into(),
    };
    s.resources.push(Resource {
        key: EntityKey::new("p", "other"),
        name: "Other".into(),
        kind: None,
        parent: None,
    });
    for index in 0..300 {
        let key = EntityKey::new("p", format!("other-{index}"));
        s.groups.push(Group {
            key: key.clone(),
            name: "Unrelated".into(),
        });
        s.memberships.push(Membership {
            member: if index == 0 {
                Subject::Account(EntityKey::new("p", "a"))
            } else {
                Subject::Group(EntityKey::new("p", format!("other-{}", index - 1)))
            },
            group: key,
            provenance: provenance(),
        });
    }
    s.grants.push(Grant {
        id: "other-grant".into(),
        subject: Subject::Group(EntityKey::new("p", "other-299")),
        resource: EntityKey::new("p", "other"),
        role: "Read".into(),
        privilege: Privilege::Standard,
        certainty: Certainty::Observed,
        evidence_kind: EvidenceKind::Assignment,
        provenance: provenance(),
    });
    assert_eq!(
        query_resource(&[s], &Aliases::new(), &[], &EntityKey::new("p", "r"))
            .unwrap()
            .access
            .len(),
        3
    );
}
