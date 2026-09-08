// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::unwrap_used)]
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(root)
        .env("PERMESH_DATA_DIR", root.join("state"))
        .env("TOKIO_WORKER_THREADS", "2")
        .args(args)
        .arg("--json")
        .output()
        .unwrap()
}
#[test]
fn adding_external_instance_is_configuration_only_and_queries_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    assert!(run(root.path(), &["init"]).status.success());
    let digest = "a".repeat(64);
    let args = [
        "provider",
        "add",
        "external",
        "--id",
        "internal-main",
        "--provider",
        "fixture",
        "--sha256",
        &digest,
        "--setting",
        "endpoint=https://example.com",
        "--credential",
        "token=env://PERMESH_EXTERNAL_MISSING",
        "--authoritative",
    ];
    assert!(run(root.path(), &args).status.success());
    let path = root.path().join("permesh.yaml");
    let before = std::fs::read(&path).unwrap();
    for args in [
        vec!["doctor"],
        vec!["user", "alice@example.com"],
        vec!["admins"],
        vec!["orphaned"],
        vec!["provider", "status"],
    ] {
        let output = run(root.path(), &args);
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["complete"], false);
    }
    assert!(!root.path().join("state").exists());
    assert_eq!(before, std::fs::read(path).unwrap());
}
#[test]
fn external_add_rejects_invalid_and_duplicate_values_without_rewriting_or_echoing() {
    let root = tempfile::tempdir().unwrap();
    run(root.path(), &["init"]);
    let path = root.path().join("permesh.yaml");
    let before = std::fs::read(&path).unwrap();
    for extra in [
        vec!["--credential", "token=SENTINEL_PRIVATE"],
        vec!["--setting", "bad"],
        vec!["--setting", "a=one", "--setting", "a=two"],
        vec!["--token-ref", "env://TOKEN"],
    ] {
        let digest = "a".repeat(64);
        let mut args = vec![
            "provider",
            "add",
            "external",
            "--provider",
            "fixture",
            "--sha256",
            &digest,
        ];
        args.extend(extra);
        let output = run(root.path(), &args);
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("SENTINEL_PRIVATE"));
        assert_eq!(before, std::fs::read(&path).unwrap());
    }
}
#[test]
fn named_environment_credentials_never_invoke_native_keychain_login() {
    let root = tempfile::tempdir().unwrap();
    run(root.path(), &["init"]);
    let digest = "a".repeat(64);
    assert!(
        run(
            root.path(),
            &[
                "provider",
                "add",
                "external",
                "--provider",
                "fixture",
                "--sha256",
                &digest,
                "--credential",
                "client_secret=env://PERMESH_EXTERNAL_MISSING"
            ]
        )
        .status
        .success()
    );
    for operation in ["login", "logout"] {
        let output = run(
            root.path(),
            &[
                "auth",
                operation,
                "external-main",
                "--credential",
                "client_secret",
            ],
        );
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stdout).contains("environment reference"));
    }
    let output = run(root.path(), &["auth", "status"]);
    assert_eq!(output.status.code(), Some(3));
    assert!(!root.path().join("state").exists());
}
