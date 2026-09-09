// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use super::*;
use serde_json::{Value, json};
use std::path::PathBuf;
fn document() -> Value {
    json!({"version":1,"scope":"engineering-directory","exported_at":"2026-09-09T10:00:00Z","complete":true,"identities":[{"id":"inventory:person:1","kind":"human","affiliation":"internal","lifecycle":"inactive"}]})
}
fn now() -> OffsetDateTime {
    OffsetDateTime::parse("2026-09-09T11:00:00Z", &Rfc3339).unwrap()
}
fn parse(value: &Value) -> Result<Observation, AppError> {
    decode(
        "roster",
        value.to_string().as_bytes(),
        "a".repeat(64),
        86400,
        now(),
    )
}
#[test]
fn explicit_classifications_never_create_verified_email_evidence() {
    let observation = parse(&document()).unwrap();
    assert!(observation.snapshot.complete);
    assert_eq!(observation.snapshot.identities[0].id, "inventory:person:1");
    assert!(
        observation.snapshot.identities[0]
            .verified_emails
            .is_empty()
    );
    assert!(observation.snapshot.accounts.is_empty());
    assert_eq!(observation.scope, "engineering-directory");
    assert_eq!(observation.exported_at, "2026-09-09T10:00:00Z");
    assert_eq!(
        observation.snapshot.identities[0].status,
        IdentityStatus::Inactive
    );
}
#[test]
fn stale_and_incomplete_authorities_never_classify_missing_accounts_as_orphans() {
    for case in ["old", "incomplete"] {
        let mut document = document();
        if case == "old" {
            document["exported_at"] = json!("2026-09-01T10:00:00Z");
        } else {
            document["complete"] = json!(false);
        }
        let observed = parse(&document).unwrap();
        assert!(!observed.snapshot.complete);
        assert_eq!(observed.snapshot.identities.len(), 1);
        let mut access = Snapshot::new("access");
        access.accounts.push(permesh_core::Account {
            key: permesh_core::EntityKey::new("access", "1"),
            login: "reused".into(),
            kind: IdentityKind::Unknown,
            status: IdentityStatus::Unknown,
            affiliation: Affiliation::Unknown,
            verified_emails: vec![],
        });
        let result = permesh_core::query_orphaned(
            &[observed.snapshot, access],
            &permesh_core::Aliases::new(),
            &["roster".into()],
        )
        .unwrap();
        assert!(!result.authority_complete);
        assert!(matches!(
            result.accounts[0].reason,
            permesh_core::OrphanReason::Unassessed
        ));
    }
}
#[test]
fn strict_records_reject_duplicates_unknown_values_emails_and_future_exports() {
    for field in ["email", "verified_emails", "employment_status"] {
        let mut d = document();
        d["identities"][0][field] = json!("SENTINEL");
        let error = parse(&d).err().unwrap();
        assert!(!error.message.contains("SENTINEL"));
    }
    for field in ["kind", "affiliation", "lifecycle"] {
        let mut d = document();
        d["identities"][0][field] = json!("SENTINEL");
        assert!(parse(&d).is_err());
    }
    let mut d = document();
    let duplicate = d["identities"][0].clone();
    d["identities"].as_array_mut().unwrap().push(duplicate);
    assert!(parse(&d).is_err());
    let mut d = document();
    d["exported_at"] = json!("2026-09-10T00:00:00Z");
    assert!(parse(&d).is_err());
    let mut d = document();
    d["version"] = json!(2);
    assert!(parse(&d).is_err());
    let bytes = document()
        .to_string()
        .replace("\"version\":1", "\"version\":1,\"version\":1");
    assert!(decode("roster", bytes.as_bytes(), "a".repeat(64), 86400, now()).is_err());
    let bytes = document().to_string().replace(
        "\"kind\":\"human\"",
        "\"kind\":\"human\",\"kind\":\"human\"",
    );
    assert!(decode("roster", bytes.as_bytes(), "a".repeat(64), 86400, now()).is_err());
}
fn fixture() -> (tempfile::TempDir, ProviderConfig, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("permesh.yaml");
    std::fs::write(&workspace, "workspace").unwrap();
    let bytes = document().to_string();
    std::fs::write(temp.path().join("inventory.json"), &bytes).unwrap();
    let provider:ProviderConfig=serde_json::from_value(json!({"id":"roster","type":"inventory","inventory":{"path":"inventory.json","sha256":digest(bytes.as_bytes())}})).unwrap();
    (temp, provider, workspace)
}
#[test]
fn bounded_pinned_regular_file_reader_rejects_changes_without_reflecting_content() {
    let (temp, provider, workspace) = fixture();
    assert!(observe_at(&provider, &workspace, now()).is_ok());
    std::fs::write(temp.path().join("inventory.json"), "SENTINEL_SECRET").unwrap();
    let error = observe_at(&provider, &workspace, now()).err().unwrap();
    assert!(!error.message.contains("SENTINEL"));
    assert!(error.message.contains("digest"));
    std::fs::write(
        temp.path().join("inventory.json"),
        vec![b' '; MAX_INVENTORY_BYTES + 1],
    )
    .unwrap();
    assert!(
        observe_at(&provider, &workspace, now())
            .err()
            .unwrap()
            .message
            .contains("8 MiB")
    );
    std::fs::remove_file(temp.path().join("inventory.json")).unwrap();
    std::fs::create_dir(temp.path().join("inventory.json")).unwrap();
    assert!(observe_at(&provider, &workspace, now()).is_err());
}
#[cfg(unix)]
#[test]
fn symlink_files_and_parent_directories_are_never_followed() {
    use std::os::unix::fs::symlink;
    let (temp, mut provider, workspace) = fixture();
    std::fs::rename(
        temp.path().join("inventory.json"),
        temp.path().join("real.json"),
    )
    .unwrap();
    symlink(
        temp.path().join("real.json"),
        temp.path().join("inventory.json"),
    )
    .unwrap();
    assert!(observe_at(&provider, &workspace, now()).is_err());
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), temp.path().join("escape")).unwrap();
    provider.inventory.as_mut().unwrap().path = "escape/inventory.json".into();
    assert!(observe_at(&provider, &workspace, now()).is_err());
}

#[test]
fn freshness_bound_applies_to_fractional_seconds_and_exact_boundary() {
    let data = document().to_string();
    let exact = OffsetDateTime::parse("2026-09-10T10:00:00Z", &Rfc3339).unwrap();
    assert!(
        decode("roster", data.as_bytes(), "a".repeat(64), 86400, exact)
            .unwrap()
            .snapshot
            .complete
    );
    assert!(
        !decode(
            "roster",
            data.as_bytes(),
            "a".repeat(64),
            86400,
            exact + time::Duration::nanoseconds(1)
        )
        .unwrap()
        .snapshot
        .complete
    );
}
