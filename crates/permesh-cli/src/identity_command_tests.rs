// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use super::*;
use permesh_core::{Account, Affiliation, Identity, IdentityKind, IdentityStatus};
fn fixture() -> (tempfile::TempDir, Captured, Vec<Snapshot>) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("permesh.yaml");
    let original = b"# preserved until approval\nversion: 1\norganization: {name: Original}\nproviders: [{id: directory, type: demo}, {id: access, type: demo}]\nidentity:\n  sources: [{provider: directory, authoritative: true}]\n".to_vec();
    std::fs::write(&path, &original).unwrap();
    let config = Config::from_bytes(&original).unwrap();
    let mut directory = Snapshot::new("directory");
    directory.identities.push(Identity {
        id: "canonical-id".into(),
        kind: IdentityKind::Human,
        status: IdentityStatus::Active,
        affiliation: Affiliation::Internal,
        verified_emails: vec![],
    });
    let mut access = Snapshot::new("access");
    access.accounts.push(Account {
        key: EntityKey::new("access", "immutable"),
        login: "current-login".into(),
        kind: IdentityKind::Unknown,
        status: IdentityStatus::Unknown,
        affiliation: Affiliation::Unknown,
        verified_emails: vec![],
    });
    (
        temp,
        Captured {
            path,
            original,
            config,
        },
        vec![directory, access],
    )
}
fn command(fingerprint: Option<String>) -> IdentityCommand {
    IdentityCommand::Map {
        instance: "access".into(),
        account: "immutable".into(),
        identity: "canonical-id".into(),
        fingerprint,
    }
}
fn captured_copy(captured: &Captured) -> Captured {
    Captured {
        path: captured.path.clone(),
        original: captured.original.clone(),
        config: captured.config.clone(),
    }
}
fn outcome() -> Outcome {
    Outcome::new("identity_review", json!({})).unwrap()
}
#[test]
fn exact_proposal_is_inert_then_saves_only_after_fresh_fingerprint_approval() {
    let (_temp, captured, snapshots) = fixture();
    let proposal = finish(
        captured_copy(&captured),
        &command(None),
        outcome(),
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    assert_eq!(std::fs::read(&captured.path).unwrap(), captured.original);
    assert_eq!(proposal.report.result["applied"], false);
    assert_eq!(proposal.report.result["comments_will_be_removed"], true);
    let fp = proposal.report.result["fingerprint"]
        .as_str()
        .unwrap()
        .to_owned();
    finish(
        captured_copy(&captured),
        &command(Some(fp)),
        outcome(),
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    let updated = Config::load(&captured.path).unwrap();
    assert_eq!(updated.organization.name, "Original");
    assert_eq!(
        updated.identity.aliases["canonical-id"]["access"],
        vec!["immutable"]
    );
    assert_eq!(
        serde_json::to_value(&updated.providers).unwrap(),
        serde_json::to_value(&captured.config.providers).unwrap()
    );
}
#[test]
fn changed_evidence_revision_partial_failure_and_cancellation_never_write() {
    for case in ["evidence", "revision", "partial", "failure", "cancel"] {
        let (_temp, captured, mut snapshots) = fixture();
        let proposal = finish(
            captured_copy(&captured),
            &command(None),
            outcome(),
            &snapshots,
            &Cancellation::new(),
        )
        .unwrap();
        let fp = proposal.report.result["fingerprint"]
            .as_str()
            .unwrap()
            .to_owned();
        let cancel = Cancellation::new();
        let mut result = outcome();
        match case {
            "evidence" => snapshots[1].accounts[0].login = "renamed".into(),
            "revision" => std::fs::write(&captured.path, b"# changed by another writer\n").unwrap(),
            "partial" => snapshots[1].complete = false,
            "failure" => {
                result.report.complete = false;
                snapshots.pop();
            }
            "cancel" => cancel.cancel(),
            _ => unreachable!(),
        }
        let before = std::fs::read(&captured.path).unwrap();
        assert!(
            finish(captured, &command(Some(fp)), result, &snapshots, &cancel).is_err(),
            "{case}"
        );
        assert_eq!(
            std::fs::read(_temp.path().join("permesh.yaml")).unwrap(),
            before
        );
    }
}
#[test]
fn proposals_never_expose_arbitrary_provider_configuration() {
    let (_temp, mut captured, snapshots) = fixture();
    // Domain review only emits allowlisted scope. Config bytes bind the fingerprint,
    // but they are not serialized as an identity proposal.
    captured.config.providers[1]=serde_json::from_value(json!({"id":"access","type":"external","external":{"provider":"custom","sha256":"a".repeat(64),"configuration":{"accidental_secret":"SENTINEL_NEVER_PRINT"}}})).unwrap();
    let proposal = finish(
        captured,
        &command(None),
        outcome(),
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    assert!(
        !serde_json::to_string(&proposal.report)
            .unwrap()
            .contains("SENTINEL_NEVER_PRINT")
    );
}
#[test]
fn authoritative_customer_scope_is_visible_and_changes_invalidate_approval() {
    let (_temp, mut captured, snapshots) = fixture();
    captured.config.providers[0] = serde_json::from_value(json!({"id":"directory","type":"external","external":{"provider":"google","sha256":"a".repeat(64),"configuration":{"customer_id":"C123"}}})).unwrap();
    captured.original = permesh_config::to_yaml(&captured.config)
        .unwrap()
        .into_bytes();
    std::fs::write(&captured.path, &captured.original).unwrap();
    let proposal = finish(
        captured_copy(&captured),
        &command(None),
        outcome(),
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    assert_eq!(
        proposal.report.result["change"]["canonical_target"]["authorities"][0]["configured_scope"]
            ["customer_id"],
        json!(["C123"])
    );
    let fp = proposal.report.result["fingerprint"]
        .as_str()
        .unwrap()
        .to_owned();
    captured.config.providers[0]
        .external
        .as_mut()
        .unwrap()
        .configuration
        .insert("customer_id".into(), json!("C456"));
    captured.original = permesh_config::to_yaml(&captured.config)
        .unwrap()
        .into_bytes();
    std::fs::write(&captured.path, &captured.original).unwrap();
    let before = captured.original.clone();
    let path = captured.path.clone();
    assert!(
        finish(
            captured,
            &command(Some(fp)),
            outcome(),
            &snapshots,
            &Cancellation::new()
        )
        .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn stale_mapping_removal_requires_exact_proposal_and_preserves_other_aliases() {
    let (_temp, mut captured, mut snapshots) = fixture();
    captured.config.identity.aliases.insert(
        "deleted-canonical".into(),
        [(
            "access".into(),
            vec!["deleted-account".into(), "keep-account".into()],
        )]
        .into(),
    );
    captured.original = permesh_config::to_yaml(&captured.config)
        .unwrap()
        .into_bytes();
    std::fs::write(&captured.path, &captured.original).unwrap();
    snapshots[0].identities.clear();
    snapshots[1].accounts.clear();
    let mut remove = IdentityCommand::Unmap {
        instance: "access".into(),
        account: "deleted-account".into(),
        identity: "deleted-canonical".into(),
        fingerprint: None,
    };
    let proposal = finish(
        captured_copy(&captured),
        &remove,
        outcome(),
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    assert_eq!(std::fs::read(&captured.path).unwrap(), captured.original);
    assert!(proposal.report.result["change"]["account_before"].is_null());
    if let IdentityCommand::Unmap { fingerprint, .. } = &mut remove {
        *fingerprint = Some(
            proposal.report.result["fingerprint"]
                .as_str()
                .unwrap()
                .into(),
        );
    }
    let path = captured.path.clone();
    finish(
        captured,
        &remove,
        outcome(),
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    assert_eq!(
        Config::load(&path).unwrap().identity.aliases["deleted-canonical"]["access"],
        vec!["keep-account"]
    );
}
#[test]
fn complete_scoped_observations_allow_mapping_but_changed_scope_needs_new_approval() {
    let (_temp, captured, mut snapshots) = fixture();
    snapshots[0].limitations = vec!["Directory does not establish employment".into()];
    snapshots[1].limitations = vec!["Only configured organizations are observed".into()];
    let proposal = finish(
        captured_copy(&captured),
        &command(None),
        outcome(),
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    assert_eq!(
        proposal.report.result["collection_scope"]["access"],
        json!(["Only configured organizations are observed"])
    );
    let fp = proposal.report.result["fingerprint"]
        .as_str()
        .unwrap()
        .to_owned();
    snapshots[1]
        .limitations
        .push("Additional scope restriction".into());
    assert!(
        finish(
            captured_copy(&captured),
            &command(Some(fp.clone())),
            outcome(),
            &snapshots,
            &Cancellation::new()
        )
        .is_err()
    );
    snapshots[1].limitations.pop();
    let saved = finish(
        captured,
        &command(Some(fp)),
        outcome(),
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    assert_eq!(saved.report.result["applied"], true);
}
#[test]
fn fresh_capture_timestamps_and_scope_order_do_not_change_semantic_fingerprint() {
    let (_temp, captured, mut snapshots) = fixture();
    snapshots[1].limitations = vec!["Scope B".into(), "Scope A".into()];
    let mut first = outcome();
    first.report.started_at = "2026-01-01T00:00:00Z".into();
    first.report.completed_at = "2026-01-01T00:00:01Z".into();
    let proposal = finish(
        captured_copy(&captured),
        &command(None),
        first,
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    snapshots[1].limitations.reverse();
    let mut second = outcome();
    second.report.started_at = "2026-02-02T00:00:00Z".into();
    second.report.completed_at = "2026-02-02T00:00:01Z".into();
    let refreshed = finish(
        captured,
        &command(None),
        second,
        &snapshots,
        &Cancellation::new(),
    )
    .unwrap();
    assert_eq!(
        proposal.report.result["fingerprint"],
        refreshed.report.result["fingerprint"]
    );
}
