// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::unwrap_used)]
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(path)
        .env("PERMESH_DATA_DIR", path.join("state"))
        .env("TOKIO_WORKER_THREADS", "2")
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn noninteractive_setup_requires_answers_before_trust_or_workspace_access() {
    let root = tempfile::tempdir().unwrap();
    let out = run(root.path(), &["provider", "setup", "fixture", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stdout).contains("--answers"));
    assert!(!root.path().join("state").exists());
}
#[test]
fn describe_needs_registration_but_not_workspace_and_never_creates_state() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("permesh.yaml"), "invalid: workspace").unwrap();
    let out = run(
        root.path(),
        &["provider", "setup", "fixture", "--describe", "--json"],
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("workspace YAML"));
    assert!(!root.path().join("state").exists());
}
#[test]
fn setup_argument_errors_never_echo_values() {
    let root = tempfile::tempdir().unwrap();
    let out = run(
        root.path(),
        &[
            "provider",
            "setup",
            "fixture",
            "--describe",
            "--answers",
            "SENTINEL",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("SENTINEL"));
}
