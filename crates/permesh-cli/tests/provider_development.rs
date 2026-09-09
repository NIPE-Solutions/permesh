// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{fs, process::Command};
#[test]
fn saved_transcript_validation_needs_no_workspace_and_never_executes_input() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("fixture.ndjson");
    fs::write(
        &path,
        include_bytes!("../../permesh-provider-protocol/tests/fixtures/negotiated-v1.ndjson"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(root.path())
        .args([
            "provider",
            "dev",
            "validate",
            "fixture.ndjson",
            "--provider",
            "example",
            "--instance",
            "work",
            "--operation",
            "discover",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["result"]["development_version"], 1);
    assert_eq!(report["result"]["executed"], false);
    assert_eq!(report["result"]["counts"]["accounts"], 1);
    fs::write(
        &path,
        b"#!/bin/sh\ntouch SHOULD_NOT_EXIST\nSENTINEL_SECRET\n",
    )
    .unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(root.path())
        .args([
            "provider",
            "dev",
            "validate",
            "fixture.ndjson",
            "--provider",
            "example",
            "--instance",
            "work",
            "--operation",
            "discover",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
    assert!(!root.path().join("SHOULD_NOT_EXIST").exists());
    assert!(!String::from_utf8_lossy(&rejected.stdout).contains("SENTINEL_SECRET"));
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("SENTINEL_SECRET"));
}
