// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_core::{identity_review::*, *};
fn fixture() -> (Vec<Snapshot>, Vec<String>) {
    let mut directory = Snapshot::new("directory");
    directory.identities = vec![
        Identity {
            id: "canonical-1".into(),
            kind: IdentityKind::Human,
            affiliation: Affiliation::Internal,
            status: IdentityStatus::Active,
            verified_emails: vec!["verified@example.com".into()],
        },
        Identity {
            id: "canonical-2".into(),
            kind: IdentityKind::Service,
            affiliation: Affiliation::External,
            status: IdentityStatus::Unknown,
            verified_emails: vec![],
        },
    ];
    let mut access = Snapshot::new("access");
    access.accounts.push(Account {
        key: EntityKey::new("access", "immutable-1"),
        login: "label".into(),
        kind: IdentityKind::Unknown,
        affiliation: Affiliation::Unknown,
        status: IdentityStatus::Unknown,
        verified_emails: vec![],
    });
    (vec![directory, access], vec!["directory".into()])
}
#[test]
fn stable_mapping_survives_rename_but_never_transfers_to_recycled_login() {
    let (mut snapshots, authorities) = fixture();
    let key = snapshots[1].accounts[0].key.clone();
    let aliases = change(
        &snapshots,
        &Aliases::new(),
        &authorities,
        &key,
        "canonical-1",
        false,
    )
    .unwrap();
    snapshots[1].accounts[0].login = "renamed".into();
    assert!(matches!(
        review(&snapshots, &aliases, &authorities).unwrap().accounts[0].resolution,
        IdentityResolution::Resolved { .. }
    ));
    snapshots[1].accounts[0].key.id = "immutable-2".into();
    snapshots[1].accounts[0].login = "label".into();
    assert!(matches!(
        review(&snapshots, &aliases, &authorities).unwrap().accounts[0].resolution,
        IdentityResolution::Unmapped
    ));
    assert!(
        change(
            &snapshots,
            &aliases,
            &authorities,
            &key,
            "canonical-1",
            true
        )
        .unwrap()
        .is_empty()
    );
}
#[test]
fn mappings_never_override_authoritative_verified_evidence_or_conflicts() {
    let (mut snapshots, authorities) = fixture();
    let key = snapshots[1].accounts[0].key.clone();
    snapshots[1].accounts[0].verified_emails = vec!["verified@example.com".into()];
    assert!(matches!(
        change(
            &snapshots,
            &Aliases::new(),
            &authorities,
            &key,
            "canonical-2",
            false
        ),
        Err(MappingError::Conflict)
    ));
    let aliases = Aliases::from([(
        "canonical-2".into(),
        std::collections::BTreeMap::from([("access".into(), vec!["immutable-1".into()])]),
    )]);
    assert!(matches!(
        review(&snapshots, &aliases, &authorities).unwrap().accounts[0].resolution,
        IdentityResolution::Ambiguous { .. }
    ));
    let mut contradictory = snapshots[0].clone();
    contradictory.provider = "other".into();
    contradictory.identities[0].status = IdentityStatus::Inactive;
    snapshots.push(contradictory);
    assert!(matches!(
        change(
            &snapshots,
            &Aliases::new(),
            &["directory".into(), "other".into()],
            &key,
            "canonical-1",
            false
        ),
        Err(MappingError::Conflict)
    ));
}
#[test]
fn explicit_target_and_fresh_complete_authorities_are_required() {
    let (mut snapshots, authorities) = fixture();
    let key = snapshots[1].accounts[0].key.clone();
    assert!(matches!(
        change(
            &snapshots,
            &Aliases::new(),
            &authorities,
            &key,
            "label",
            false
        ),
        Err(MappingError::TargetMissing)
    ));
    snapshots[0].complete = false;
    assert!(matches!(
        change(
            &snapshots,
            &Aliases::new(),
            &authorities,
            &key,
            "canonical-1",
            false
        ),
        Err(MappingError::Incomplete)
    ));
    snapshots.remove(0);
    assert!(matches!(
        change(
            &snapshots,
            &Aliases::new(),
            &authorities,
            &key,
            "canonical-1",
            false
        ),
        Err(MappingError::Incomplete)
    ));
}
