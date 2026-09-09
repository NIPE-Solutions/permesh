// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{fs, process::Command};

#[test]
fn guided_add_requires_explicit_noninteractive_inputs_before_network_or_state() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("permesh.yaml"),
        "version: 1\norganization: {name: Test}\nproviders: []\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(root.path())
        .env("PERMESH_DATA_DIR", root.path().join("state"))
        .args(["provider", "add", "github", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--answers FILE and --accept-risk")
    );
    assert!(output.stderr.is_empty());
    assert!(!root.path().join("state").exists());
}

#[test]
fn invalid_official_and_external_add_flags_fail_before_network() {
    let root = tempfile::tempdir().unwrap();
    for args in [
        vec![
            "provider",
            "add",
            "google",
            "--customer-id",
            "C123",
            "--accept-risk",
        ],
        vec!["provider", "add", "external", "--answers", "unused"],
        vec!["provider", "add", "google", "--version", "1.0.0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_permesh"))
            .current_dir(root.path())
            .env("PERMESH_DATA_DIR", root.path().join("state"))
            .args(&args)
            .arg("--json")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            report["error"]["message"]
                .as_str()
                .unwrap()
                .contains(if args[2] == "external" {
                    "require provider add"
                } else if args.contains(&"--customer-id") {
                    "declarative form"
                } else {
                    "--answers FILE and --accept-risk"
                })
        );
        assert!(output.stderr.is_empty());
        assert!(!root.path().join("state").exists());
    }
}
