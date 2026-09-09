// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_external::trust::{Registry, inspect};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(root)
        .env("PERMESH_DATA_DIR", root.join("data"))
        .env_remove("PERMESH_DIAGNOSTICS_MISSING_4312")
        .args(args)
        .output()
        .unwrap()
}
fn doctor(root: &Path) -> Value {
    let output = run(root, &["doctor", "--details", "--json"]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn detailed_preflight_stops_at_approval_then_credentials_then_network() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let executable = root.join("native");
    // Native magic fixture is inspected/trusted but never executed: each case stops locally.
    std::fs::write(&executable, b"\x7fELFdiagnostics fixture").unwrap();
    let digest = inspect(&executable).unwrap().sha256;
    Registry::new(root.join("data/providers"))
        .unwrap()
        .trust(&executable, "fixture", &digest, &[])
        .unwrap();
    let mut config = json!({"version":1,"organization":{"name":"test"},"providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":digest,"discovery_protocol":"negotiated_v1","credentials":{"token":"env://PERMESH_DIAGNOSTICS_MISSING_4312"}}}]});
    let path = root.join("permesh.yaml");
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let report = doctor(&root);
    assert_eq!(
        report["result"]["diagnostics"][0]["code"],
        "approval_missing_or_stale"
    );
    let reviewed = run(
        &root,
        &["provider", "external", "review", "instance", "--json"],
    );
    assert!(reviewed.status.success());
    let reviewed: Value = serde_json::from_slice(&reviewed.stdout).unwrap();
    let fingerprint = reviewed["result"]["fingerprint"].as_str().unwrap();
    assert!(
        run(
            &root,
            &[
                "provider",
                "external",
                "approve",
                "instance",
                "--fingerprint",
                fingerprint,
                "--accept-risk"
            ]
        )
        .status
        .success()
    );
    let report = doctor(&root);
    assert_eq!(
        report["result"]["diagnostics"][0]["code"],
        "credential_unavailable"
    );
    config["providers"][0]["external"]["network"] =
        json!({"ca_bundle":{"path":"missing.pem","sha256":"b".repeat(64)}});
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let report = doctor(&root);
    assert_eq!(report["result"]["diagnostics"][0]["stage"], "network");
    assert_eq!(
        report["result"]["diagnostics"][0]["code"],
        "network_context_invalid"
    );
    assert!(
        !report["result"]["diagnostics"]
            .to_string()
            .contains("PERMESH_DIAGNOSTICS_MISSING_4312")
    );
    assert!(
        !report["result"]["diagnostics"]
            .to_string()
            .contains("missing.pem")
    );
}
