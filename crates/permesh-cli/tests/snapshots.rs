// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap()
}
fn json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn snapshot_is_explicit_private_and_readable_without_workspace() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    let before = fs::read(dir.path().join("permesh.yaml")).unwrap();
    let created = json(run(
        dir.path(),
        &["snapshot", "create", "--output", "review.json", "--json"],
    ));
    assert_eq!(created["result"]["format_version"], 1);
    let bytes = fs::read(dir.path().join("review.json")).unwrap();
    let saved: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved["format"], "permesh_snapshot");
    assert_eq!(
        saved["providers"][0]["data"]["accounts"][0]["key"]["provider"],
        "demo"
    );
    assert_eq!(
        run(
            dir.path(),
            &["snapshot", "create", "--output", "review.json"]
        )
        .status
        .code(),
        Some(2)
    );
    assert_eq!(fs::read(dir.path().join("review.json")).unwrap(), bytes);
    assert_eq!(fs::read(dir.path().join("permesh.yaml")).unwrap(), before);
    fs::remove_file(dir.path().join("permesh.yaml")).unwrap();
    json(run(
        dir.path(),
        &["snapshot", "inspect", "review.json", "--json"],
    ));
    let diff = json(run(
        dir.path(),
        &["diff", "review.json", "review.json", "--json"],
    ));
    assert_eq!(diff["result"]["changes"], serde_json::json!([]));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(dir.path().join("review.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o077,
            0
        );
    }
}
#[test]
fn partial_snapshot_absence_is_inconclusive_not_revocation() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    json(run(
        dir.path(),
        &["snapshot", "create", "--output", "before.json", "--json"],
    ));
    let mut after: Value =
        serde_json::from_slice(&fs::read(dir.path().join("before.json")).unwrap()).unwrap();
    after["providers"][0]["state"] = "partial".into();
    after["providers"][0]["data"]["complete"] = false.into();
    after["providers"][0]["data"]["grants"] = serde_json::json!([]);
    fs::write(
        dir.path().join("after.json"),
        serde_json::to_vec(&after).unwrap(),
    )
    .unwrap();
    let output = run(dir.path(), &["diff", "before.json", "after.json", "--json"]);
    assert_eq!(output.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["complete"], false);
    assert!(
        report["result"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["change"] != "no_longer_observed")
    );
    assert!(
        !report["result"]["inconclusive"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn malformed_unknown_and_duplicate_artifact_fields_fail_without_reflection() {
    let dir = tempfile::tempdir().unwrap();
    for content in [
        r#"{"format":"permesh_snapshot","format_version":999}"#,
        r#"{"format":"SECRET_REFLECTION","format":"x"}"#,
    ] {
        fs::write(dir.path().join("bad.json"), content).unwrap();
        let output = run(dir.path(), &["snapshot", "inspect", "bad.json", "--json"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("SECRET_REFLECTION"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET_REFLECTION"));
    }
}

fn pair() -> (tempfile::TempDir, Value) {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    json(run(
        dir.path(),
        &["snapshot", "create", "--output", "before.json", "--json"],
    ));
    let v = serde_json::from_slice(&fs::read(dir.path().join("before.json")).unwrap()).unwrap();
    (dir, v)
}
fn changed(dir: &Path, value: &Value) -> Output {
    let mut value = value.clone();
    let next = time::OffsetDateTime::parse(
        value["completed_at"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
        + time::Duration::seconds(1);
    let stamp = next
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    value["started_at"] = stamp.clone().into();
    value["completed_at"] = stamp.clone().into();
    for p in value["providers"].as_array_mut().unwrap() {
        p["started_at"] = stamp.clone().into();
        p["completed_at"] = stamp.clone().into();
    }
    fs::write(dir.join("after.json"), serde_json::to_vec(&value).unwrap()).unwrap();
    run(dir, &["diff", "before.json", "after.json", "--json"])
}
#[test]
fn reordering_and_new_observation_times_are_not_access_changes() {
    let (dir, mut value) = pair();
    for key in [
        "accounts",
        "resources",
        "grants",
        "identities",
        "memberships",
        "groups",
    ] {
        value["providers"][0]["data"][key]
            .as_array_mut()
            .unwrap()
            .reverse();
    }
    for grant in value["providers"][0]["data"]["grants"]
        .as_array_mut()
        .unwrap()
    {
        grant["provenance"]["observed_at"] = "2026-09-09T00:00:00Z".into();
    }
    assert_eq!(
        json(changed(dir.path(), &value))["result"]["changes"],
        serde_json::json!([])
    );
}
#[test]
fn stable_account_rename_and_native_role_change_retain_identity() {
    let (dir, mut value) = pair();
    value["providers"][0]["data"]["accounts"][0]["login"] = "renamed-login".into();
    value["providers"][0]["data"]["grants"][0]["role"] = "reviewed-native-role".into();
    let report = json(changed(dir.path(), &value));
    let changes = report["result"]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().all(|r| r["change"] == "changed"));
}
#[test]
fn changed_scope_and_mapping_are_separate_from_access_removal() {
    let (dir, mut value) = pair();
    value["identity"]["bindings"] = serde_json::json!([{ "identity":"reviewed-new-canonical", "instance":"demo", "account":"alice" }]);
    let report = json(changed(dir.path(), &value));
    assert_eq!(report["result"]["identity_context_changed"], true);
    assert_eq!(report["result"]["changes"], serde_json::json!([]));
    value["providers"][0]["context_sha256"] = "b".repeat(64).into();
    value["providers"][0]["data"]["grants"] = serde_json::json!([]);
    let report = changed(dir.path(), &value);
    assert_eq!(report.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&report.stdout).unwrap();
    assert!(
        report["result"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["change"] == "missing_unconfirmed")
    );
}
#[test]
fn unsupported_capabilities_and_duplicate_nested_keys_are_rejected() {
    let (dir, mut value) = pair();
    value["providers"][0]["capabilities"] = serde_json::json!([]);
    fs::write(
        dir.path().join("bad.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    assert_eq!(
        run(dir.path(), &["snapshot", "inspect", "bad.json", "--json"])
            .status
            .code(),
        Some(2)
    );
    let good = fs::read_to_string(dir.path().join("before.json")).unwrap();
    let duplicate = good.replacen(
        "\"format_version\": 1",
        "\"format_version\": 1, \"format_version\": 1",
        1,
    );
    fs::write(dir.path().join("bad.json"), duplicate).unwrap();
    assert_eq!(
        run(dir.path(), &["snapshot", "inspect", "bad.json", "--json"])
            .status
            .code(),
        Some(2)
    );
}
#[test]
fn oversized_files_are_rejected_before_parsing() {
    let dir = tempfile::tempdir().unwrap();
    let file = fs::File::create(dir.path().join("huge.json")).unwrap();
    file.set_len(64 * 1024 * 1024 + 1).unwrap();
    assert_eq!(
        run(dir.path(), &["snapshot", "inspect", "huge.json", "--json"])
            .status
            .code(),
        Some(2)
    );
}
#[cfg(unix)]
#[test]
fn snapshot_writes_never_follow_symlink_destinations() {
    let (dir, _) = pair();
    std::os::unix::fs::symlink("before.json", dir.path().join("link.json")).unwrap();
    let before = fs::read(dir.path().join("before.json")).unwrap();
    assert_eq!(
        run(
            dir.path(),
            &[
                "snapshot",
                "create",
                "--output",
                "link.json",
                "--overwrite",
                "--json"
            ]
        )
        .status
        .code(),
        Some(2)
    );
    assert_eq!(fs::read(dir.path().join("before.json")).unwrap(), before);
}
#[test]
fn withdrawing_identity_authority_is_not_upstream_identity_removal() {
    let (dir, mut value) = pair();
    value["identity"]["authorities"] = serde_json::json!([]);
    value["providers"][0]["data"]["identities"] = serde_json::json!([]);
    let output = changed(dir.path(), &value);
    assert_eq!(output.status.code(), Some(4));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["result"]["changes"].as_array().unwrap().is_empty());
    assert!(
        report["result"]["inconclusive"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["reason"] == "identity_authority_changed")
    );
}

#[test]
fn reversed_or_overlapping_capture_windows_make_absence_inconclusive() {
    let (dir, mut value) = pair();
    value["providers"][0]["data"]["grants"] = serde_json::json!([]);
    // Exact same capture window but different observations is not a later successful scan.
    fs::write(
        dir.path().join("overlap.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    let output = run(
        dir.path(),
        &["diff", "before.json", "overlap.json", "--json"],
    );
    assert_eq!(output.status.code(), Some(4));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        result["result"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["change"] == "missing_unconfirmed")
    );
}
fn inventory_export(
    directory: &Path,
    scope: &str,
    exported_at: time::OffsetDateTime,
    present: bool,
) {
    use sha2::{Digest, Sha256};
    let identities = if present {
        serde_json::json!([{"id":"person:1","kind":"human","affiliation":"internal","lifecycle":"active"}])
    } else {
        serde_json::json!([])
    };
    let data = serde_json::json!({"version":1,"scope":scope,"exported_at":exported_at.format(&time::format_description::well_known::Rfc3339).unwrap(),"complete":true,"identities":identities});
    let bytes = serde_json::to_vec(&data).unwrap();
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    fs::write(directory.join("inventory.json"), bytes).unwrap();
    let config = serde_json::json!({"version":1,"organization":{"name":"Example"},"providers":[{"id":"roster","type":"inventory","inventory":{"path":"inventory.json","sha256":digest}}],"identity":{"sources":[{"provider":"roster","authoritative":true}]}});
    fs::write(
        directory.join("permesh.yaml"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
}
#[test]
fn newer_pinned_export_is_data_change_but_older_or_rescoped_export_is_inconclusive() {
    let directory = tempfile::tempdir().unwrap();
    let d = directory.path();
    let now = time::OffsetDateTime::now_utc();
    inventory_export(d, "reviewed-team", now - time::Duration::seconds(100), true);
    json(run(
        d,
        &["snapshot", "create", "--output", "before.json", "--json"],
    ));
    inventory_export(d, "reviewed-team", now - time::Duration::seconds(50), false);
    json(run(
        d,
        &["snapshot", "create", "--output", "after.json", "--json"],
    ));
    let comparison = json(run(d, &["diff", "before.json", "after.json", "--json"]));
    assert_eq!(
        comparison["result"]["changes"][0]["change"],
        "no_longer_observed"
    );
    inventory_export(
        d,
        "reviewed-team",
        now - time::Duration::seconds(150),
        false,
    );
    json(run(
        d,
        &["snapshot", "create", "--output", "older.json", "--json"],
    ));
    assert_eq!(
        run(d, &["diff", "before.json", "older.json", "--json"])
            .status
            .code(),
        Some(4)
    );
    inventory_export(
        d,
        "different-team",
        now - time::Duration::seconds(40),
        false,
    );
    json(run(
        d,
        &["snapshot", "create", "--output", "rescoped.json", "--json"],
    ));
    assert_eq!(
        run(d, &["diff", "before.json", "rescoped.json", "--json"])
            .status
            .code(),
        Some(4)
    );
}

#[test]
fn missing_inventory_is_a_recorded_failure_not_an_empty_success() {
    let directory = tempfile::tempdir().unwrap();
    let d = directory.path();
    inventory_export(
        d,
        "reviewed-team",
        time::OffsetDateTime::now_utc() - time::Duration::seconds(1),
        true,
    );
    fs::remove_file(d.join("inventory.json")).unwrap();
    let output = run(
        d,
        &["snapshot", "create", "--output", "failed.json", "--json"],
    );
    assert_eq!(output.status.code(), Some(3));
    let value: Value = serde_json::from_slice(&fs::read(d.join("failed.json")).unwrap()).unwrap();
    assert_eq!(value["providers"][0]["state"], "failed");
    assert_eq!(
        value["providers"][0]["failure_code"],
        "provider_collection_failed"
    );
    assert!(value["providers"][0]["data"].is_null());
    assert_eq!(
        run(d, &["snapshot", "inspect", "failed.json", "--json"])
            .status
            .code(),
        Some(4)
    );
}
#[test]
fn contradictory_limitations_and_missing_inventory_provenance_are_rejected() {
    let (directory, mut value) = pair();
    value["providers"][0]["limitations"] = serde_json::json!(["hidden_scope"]);
    assert_eq!(changed(directory.path(), &value).status.code(), Some(2));
    let directory = tempfile::tempdir().unwrap();
    let d = directory.path();
    inventory_export(
        d,
        "reviewed-team",
        time::OffsetDateTime::now_utc() - time::Duration::seconds(1),
        true,
    );
    json(run(
        d,
        &["snapshot", "create", "--output", "before.json", "--json"],
    ));
    let mut value: Value =
        serde_json::from_slice(&fs::read(d.join("before.json")).unwrap()).unwrap();
    value["providers"][0]["source_observation"] = Value::Null;
    assert_eq!(changed(d, &value).status.code(), Some(2));
}

#[test]
fn version_one_fixture_survives_storage_and_domain_refactors() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = include_bytes!("fixtures/snapshot-v1.json");
    fs::write(dir.path().join("fixture.json"), bytes).unwrap();
    let result = json(run(
        dir.path(),
        &["snapshot", "inspect", "fixture.json", "--json"],
    ));
    let fixture: Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(result["result"], fixture);
    assert_eq!(
        json(run(
            dir.path(),
            &["diff", "fixture.json", "fixture.json", "--json"]
        ))["result"]["changes"],
        serde_json::json!([])
    );
}
