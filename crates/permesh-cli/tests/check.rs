// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("PERMESH_DATA_DIR", dir.join("state"))
        .args(args)
        .output()
        .unwrap()
}
fn demo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    dir
}
fn report(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn demo_check_reports_departed_access_and_exits_6() {
    let dir = demo();
    let output = run(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--json"],
    );
    assert_eq!(output.status.code(), Some(6));
    let report = report(&output);
    assert_eq!(report["command"], "check");
    let result = &report["result"];
    assert_eq!(result["check_report_version"], 1);
    assert_eq!(result["complete"], true);
    assert_eq!(result["counts"]["departed_with_access"], 1);
    assert_eq!(result["counts"]["policy_finding"], 0);
    assert_eq!(result["changes"], "no_baseline");
    for file in ["snapshot.json", "check.json", "summary.md"] {
        assert!(dir.path().join("out").join(file).is_file(), "{file}");
    }
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("out/check.json")).unwrap()).unwrap();
    assert_eq!(saved["counts"], result["counts"]);
    assert!(
        run(dir.path(), &["snapshot", "inspect", "out/snapshot.json"])
            .status
            .success()
    );
}

#[test]
fn summary_contains_counts_but_no_names() {
    let dir = demo();
    run(dir.path(), &["check", "--out", "out", "--no-notify"]);
    let summary = fs::read_to_string(dir.path().join("out/summary.md")).unwrap();
    assert!(
        summary.contains("Departed people with access: 1"),
        "{summary}"
    );
    for name in ["former-dev", "alice", "bob", "@example.com", "acme/"] {
        assert!(!summary.contains(name), "{name} in {summary}");
    }
}

#[test]
fn human_output_lists_counts_and_files() {
    let dir = demo();
    let output = run(dir.path(), &["check", "--out", "out", "--no-notify"]);
    assert_eq!(output.status.code(), Some(6));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Departed people with access: 1"), "{text}");
    assert!(text.contains("summary.md"), "{text}");
}

#[test]
fn existing_output_requires_overwrite() {
    let dir = demo();
    run(dir.path(), &["check", "--out", "out", "--no-notify"]);
    let before = fs::read(dir.path().join("out/check.json")).unwrap();
    let again = run(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--json"],
    );
    assert_eq!(again.status.code(), Some(2));
    assert_eq!(fs::read(dir.path().join("out/check.json")).unwrap(), before);
    let forced = run(
        dir.path(),
        &["check", "--out", "out", "--no-notify", "--overwrite"],
    );
    assert_eq!(forced.status.code(), Some(6));
}

#[test]
fn out_cannot_be_workspace_directory() {
    let dir = demo();
    let before = fs::read(dir.path().join("permesh.yaml")).unwrap();
    let output = run(
        dir.path(),
        &["check", "--out", ".", "--no-notify", "--json"],
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(fs::read(dir.path().join("permesh.yaml")).unwrap(), before);
    assert!(!dir.path().join("check.json").exists());
}

#[test]
fn explicit_policy_file_is_used_and_departed_is_independent_of_it() {
    let dir = demo();
    fs::write(
        dir.path().join("policy.json"),
        r#"{"version":1,"rules":["external_privileged"]}"#,
    )
    .unwrap();
    let output = run(
        dir.path(),
        &[
            "check",
            "--out",
            "out",
            "--policy",
            "policy.json",
            "--no-notify",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(6));
    let result = &report(&output)["result"];
    assert_eq!(result["counts"]["policy_finding"], 0);
    assert_eq!(result["counts"]["departed_with_access"], 1);
    assert_eq!(
        result["policy"]["rules"],
        serde_json::json!(["external_privileged"])
    );
}
