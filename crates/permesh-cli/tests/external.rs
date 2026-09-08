// SPDX-License-Identifier: MIT
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

#[test]
fn default_storage_uses_native_nonroaming_locations_without_creating_them() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .args(["provider", "external", "list", "--json"])
        .current_dir(&root)
        .env_remove("PERMESH_DATA_DIR")
        .env("HOME", &root)
        .env("USERPROFILE", &root)
        .env("LOCALAPPDATA", &root)
        .env("XDG_DATA_HOME", &root)
        .env("TOKIO_WORKER_THREADS", "2")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    #[cfg(windows)]
    let expected = root.join("permesh").join("data").join("providers");
    #[cfg(target_os = "macos")]
    let expected = root.join("Library/Application Support/permesh/providers");
    #[cfg(not(any(windows, target_os = "macos")))]
    let expected = root.join("permesh/providers");
    assert_eq!(report["result"]["storage"], expected.to_str().unwrap());
    assert!(!expected.exists());
    assert!(out.stderr.is_empty());
}

#[test]
fn external_discovery_reports_schema_two_with_legacy_unknowns_and_partial_snapshot() {
    use sha2::{Digest, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let source = root.join("peer.rs");
    let binary = root.join(if cfg!(windows) { "peer.exe" } else { "peer" });
    std::fs::write(&source, r##"
use std::io::{BufRead, Write};
fn main() {
    let mut lines = std::io::stdin().lock().lines();
    lines.next().unwrap().unwrap();
    println!("{}", r#"{"protocol":1,"id":"handshake","event":"handshake","provider":"fixture","capabilities":["accounts","resources","grants"],"draft":true}"#);
    std::io::stdout().flush().unwrap();
    lines.next().unwrap().unwrap();
    println!("{}", r#"{"protocol":1,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"instance","id":"guest"},"login":"guest","kind":"external","verified_emails":[]}}"#);
    println!("{}", r#"{"protocol":1,"id":"discover","event":"record","kind":"resource","data":{"key":{"provider":"instance","id":"r"},"name":"Resource"}}"#);
    println!("{}", r#"{"protocol":1,"id":"discover","event":"record","kind":"grant","data":{"id":"g","subject":{"kind":"account","key":{"provider":"instance","id":"guest"}},"resource":{"provider":"instance","id":"r"},"role":"Reader","privilege":"standard","certainty":"observed","provenance":{"method":"fixture","observed_at":"2026-01-01T00:00:00Z"}}}"#);
    println!("{}", r#"{"protocol":1,"id":"discover","event":"complete","count":3,"complete":false,"limitations":["visibility_limited"]}"#);
    std::io::stdout().flush().unwrap();
    assert!(lines.next().is_none());
}
"##).unwrap();
    assert!(
        Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let digest = Sha256::digest(std::fs::read(&binary).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let trust = run(
        &root,
        &[
            "provider",
            "external",
            "trust",
            binary.to_str().unwrap(),
            "--id",
            "fixture",
            "--sha256",
            &digest,
            "--capability",
            "accounts",
            "--capability",
            "resources",
            "--capability",
            "grants",
            "--accept-risk",
            "--json",
        ],
    );
    assert!(trust.status.success(), "{trust:?}");
    let value: serde_json::Value = serde_json::from_slice(&trust.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    let output = run(
        &root,
        &[
            "provider",
            "external",
            "discover",
            "fixture",
            "--instance",
            "instance",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(4), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], 2);
    assert_eq!(report["command"], "external_discover");
    assert_eq!(report["complete"], false);
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/schema2/external-snapshot.json")).unwrap();
    assert_eq!(report["result"]["snapshot"], expected);
    let failed = run(
        &root,
        &[
            "provider",
            "external",
            "discover",
            "missing",
            "--instance",
            "instance",
            "--json",
        ],
    );
    assert!(!failed.status.success());
    let error: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(error["schema_version"], 2);
}
