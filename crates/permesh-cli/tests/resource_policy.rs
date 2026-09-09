// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use serde_json::{Value, json};
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
fn value(output: Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    assert!(
        run(
            dir.path(),
            &["snapshot", "create", "--output", "review.json"]
        )
        .status
        .success()
    );
    dir
}
#[test]
fn resource_labels_require_explicit_disambiguation_and_offline_ids_preserve_paths() {
    let dir = fixture();
    let path = dir.path().join("review.json");
    let mut saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for resource in saved["providers"][0]["data"]["resources"]
        .as_array_mut()
        .unwrap()
    {
        resource["name"] = json!("same label");
    }
    std::fs::write(&path, saved.to_string()).unwrap();
    std::fs::remove_file(dir.path().join("permesh.yaml")).unwrap();
    let ambiguous = run(
        dir.path(),
        &[
            "resource",
            "same label",
            "--snapshot",
            "review.json",
            "--json",
        ],
    );
    assert_eq!(ambiguous.status.code(), Some(2));
    assert_eq!(value(ambiguous)["result"]["selection_required"], true);
    let selected = run(
        dir.path(),
        &[
            "resource",
            "--instance",
            "demo",
            "--id",
            "repo-20",
            "--snapshot",
            "review.json",
            "--json",
        ],
    );
    assert!(selected.status.success());
    let data = value(selected);
    assert!(
        data["result"]["access"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["certainty"] == "derived")
    );
    assert_eq!(data["result"]["resource"]["key"]["id"], "repo-20");
    let human = run(
        dir.path(),
        &[
            "resource",
            "--instance",
            "demo",
            "--id",
            "repo-20",
            "--snapshot",
            "review.json",
        ],
    );
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("Access evidence"));
    assert!(text.contains("Via"));
    assert!(!text.contains("\"resource_review_version\""));
}
#[test]
fn policy_exit_contract_distinguishes_findings_partial_and_invalid_offline_input() {
    let dir = fixture();
    std::fs::write(
        dir.path().join("rules.json"),
        json!({"version":1,"rules":["inactive_access"]}).to_string(),
    )
    .unwrap();
    let result = run(
        dir.path(),
        &[
            "policy",
            "check",
            "--rules",
            "rules.json",
            "--snapshot",
            "review.json",
            "--json",
        ],
    );
    assert_eq!(result.status.code(), Some(6));
    assert!(value(result)["result"]["findings"].as_u64().unwrap() > 0);
    let path = dir.path().join("review.json");
    let mut saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    saved["providers"][0]["state"] = json!("partial");
    saved["providers"][0]["data"]["complete"] = json!(false);
    std::fs::write(&path, saved.to_string()).unwrap();
    let result = run(
        dir.path(),
        &[
            "policy",
            "check",
            "--rules",
            "rules.json",
            "--snapshot",
            "review.json",
            "--json",
        ],
    );
    assert_eq!(result.status.code(), Some(4));
    let data = value(result);
    assert!(data["result"]["findings"].as_u64().unwrap() > 0);
    assert_eq!(data["result"]["clean"], false);
    std::fs::write(
        dir.path().join("rules.json"),
        "{\"version\":1,\"version\":1,\"rules\":[\"inactive_access\"]}",
    )
    .unwrap();
    let result = run(
        dir.path(),
        &[
            "policy",
            "check",
            "--rules",
            "rules.json",
            "--snapshot",
            "review.json",
            "--json",
        ],
    );
    assert_eq!(result.status.code(), Some(2));
}
#[test]
fn fresh_policy_and_resource_commands_do_not_write_review_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    std::fs::write(
        dir.path().join("rules.json"),
        "{\"version\":1,\"rules\":[\"inactive_access\"]}",
    )
    .unwrap();
    let before = std::fs::read_dir(dir.path()).unwrap().count();
    assert!(
        run(
            dir.path(),
            &[
                "resource",
                "--instance",
                "demo",
                "--id",
                "repo-20",
                "--json"
            ]
        )
        .status
        .success()
    );
    assert_eq!(
        run(dir.path(), &["policy", "check", "--rules", "rules.json"])
            .status
            .code(),
        Some(6)
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), before);
}
