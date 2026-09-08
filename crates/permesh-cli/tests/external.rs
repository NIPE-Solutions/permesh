// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::unwrap_used)]
use std::{path::Path, process::Command};
fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .args(args)
        .current_dir(root)
        .env("PERMESH_DATA_DIR", root.join("local-state"))
        .env("TOKIO_WORKER_THREADS", "2")
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}
#[test]
fn external_list_is_local_and_does_not_create_storage_or_load_workspace() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    std::fs::write(root.join("permesh.yaml"), "malicious: invalid config").unwrap();
    let out = run(&root, &["provider", "external", "list", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["result"]["registrations"], serde_json::json!([]));
    assert!(!root.join("local-state").exists());
    assert!(out.stderr.is_empty());
}
#[test]
fn trust_needs_explicit_risk_acknowledgement_and_inspection_rejects_scripts() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path().canonicalize().unwrap();
    let script = root.join("provider.sh");
    std::fs::write(&script, "#!/bin/sh\ntouch should-not-exist\n").unwrap();
    for args in [
        vec![
            "provider",
            "external",
            "trust",
            script.to_str().unwrap(),
            "--id",
            "test",
            "--sha256",
            &"0".repeat(64),
            "--json",
        ],
        vec![
            "provider",
            "external",
            "inspect",
            script.to_str().unwrap(),
            "--json",
        ],
    ] {
        let out = run(&root, &args);
        assert_eq!(out.status.code(), Some(2));
        assert!(
            serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["error"].is_object()
        );
        assert!(out.stderr.is_empty());
    }
    assert!(!root.join("should-not-exist").exists());
    assert!(!root.join("local-state").exists());
}
