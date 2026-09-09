// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
fn json(output: Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}
fn fixture(stale: bool, authoritative: bool) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let export = if stale {
        "2020-01-01T00:00:00Z".into()
    } else {
        time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };
    let data = json!({"version":1,"scope":"explicit-test-roster","exported_at":export,"complete":true,"identities":[{"id":"roster:person-1","kind":"human","affiliation":"internal","lifecycle":"inactive"}]});
    let bytes = data.to_string();
    let digest = Sha256::digest(bytes.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    std::fs::write(temp.path().join("roster.json"), bytes).unwrap();
    let config = json!({"version":1,"organization":{"name":"Test"},"providers":[{"id":"roster","type":"inventory","inventory":{"path":"roster.json","sha256":digest}},{"id":"access","type":"demo"}],"identity":{"sources":[{"provider":"roster","authoritative":authoritative}],"aliases":{"roster:person-1":{"access":["100"]}}}});
    std::fs::write(temp.path().join("permesh.yaml"), config.to_string()).unwrap();
    temp
}
#[test]
fn inventory_authority_requires_explicit_selection_and_preserves_lifecycle_classification() {
    let temp = fixture(false, true);
    let result = run(temp.path(), &["orphaned", "--json"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let data = json(result);
    assert_eq!(data["result"]["authority_complete"], true);
    assert!(data["result"]["accounts"].as_array().unwrap().iter().any(|row|row["account"]["key"]["id"]=="100" && row["reason"]=="inactive_identity"));
    let temp = fixture(false, false);
    let result = run(temp.path(), &["orphaned", "--json"]);
    assert_eq!(result.status.code(), Some(2));
    let data = json(run(temp.path(), &["identity", "unresolved", "--json"]));
    assert!(
        data["result"]["canonical_identities"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn stale_inventory_is_partial_in_queries_failed_in_health_and_never_claims_offboarding() {
    let temp = fixture(true, true);
    let result = run(temp.path(), &["orphaned", "--json"]);
    assert_eq!(result.status.code(), Some(4));
    let data = json(result);
    assert_eq!(data["result"]["authority_complete"], false);
    assert!(
        data["result"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["reason"] == "unassessed")
    );
    let result = run(temp.path(), &["doctor", "--json"]);
    assert!(!result.status.success());
    let data = json(result);
    assert!(
        data["providers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "roster" && p["state"] == "failed")
    );
}
#[test]
fn inventory_metadata_never_reads_source_and_changed_pin_fails_without_data_reflection() {
    let temp = fixture(false, true);
    std::fs::write(temp.path().join("roster.json"), "SENTINEL_PRIVATE_CONTENT").unwrap();
    for args in [
        &["provider", "list", "--json"][..],
        &["provider", "capabilities", "roster", "--json"][..],
    ] {
        let result = run(temp.path(), args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert!(!String::from_utf8_lossy(&result.stdout).contains("SENTINEL"));
    }
    let result = run(temp.path(), &["doctor", "--json"]);
    assert!(!result.status.success());
    assert!(!String::from_utf8_lossy(&result.stdout).contains("SENTINEL"));
    assert!(String::from_utf8_lossy(&result.stdout).contains("digest"));
}

#[test]
fn snapshot_retains_typed_inventory_provenance_and_export_completeness() {
    for stale in [false, true] {
        let temp = fixture(stale, true);
        let output = run(
            temp.path(),
            &["snapshot", "create", "--output", "snapshot.json", "--json"],
        );
        assert_eq!(
            output.status.code(),
            Some(if stale { 4 } else { 0 }),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let artifact: Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("snapshot.json")).unwrap())
                .unwrap();
        let config: Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("permesh.yaml")).unwrap())
                .unwrap();
        let source: Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("roster.json")).unwrap())
                .unwrap();
        let provider = artifact["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["instance"] == "roster")
            .unwrap();
        assert_eq!(provider["source_observation"]["method"], "file_inventory");
        assert_eq!(
            provider["source_observation"]["declared_scope"],
            "explicit-test-roster"
        );
        assert_eq!(
            provider["source_observation"]["exported_at"],
            source["exported_at"]
        );
        assert_eq!(
            provider["source_observation"]["content_sha256"],
            config["providers"][0]["inventory"]["sha256"]
        );
        assert_eq!(
            provider["state"],
            if stale { "partial" } else { "complete" }
        );
    }
}
