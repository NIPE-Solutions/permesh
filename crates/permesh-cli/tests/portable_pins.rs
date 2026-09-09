// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::process::Command;
fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn expert_target_pins_are_written_without_execution_and_invalid_inputs_preserve_workspace() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        run(dir.path(), &["init", "--organization", "Example"])
            .status
            .success()
    );
    let path = dir.path().join("permesh.yaml");
    let original = std::fs::read(&path).unwrap();
    let linux = format!("x86_64-unknown-linux-gnu={}", "a".repeat(64));
    let mac = format!("aarch64-apple-darwin={}", "b".repeat(64));
    for extra in [
        vec![
            "--target-sha256",
            linux.as_str(),
            "--target-sha256",
            linux.as_str(),
        ],
        vec!["--target-sha256", "unknown=bad"],
        vec!["--sha256", "bad", "--target-sha256", linux.as_str()],
    ] {
        let mut args = vec![
            "provider",
            "add",
            "external",
            "--id",
            "example",
            "--provider",
            "uninstalled",
        ];
        args.extend(extra);
        assert_eq!(run(dir.path(), &args).status.code(), Some(2));
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    let output = run(
        dir.path(),
        &[
            "provider",
            "add",
            "external",
            "--id",
            "example",
            "--provider",
            "uninstalled",
            "--target-sha256",
            &linux,
            "--target-sha256",
            &mac,
            "--json",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = permesh_config::Config::load(&path).unwrap();
    let value = serde_json::to_value(config.providers[0].external.as_ref().unwrap()).unwrap();
    assert!(value.get("sha256").is_none());
    assert_eq!(
        value["sha256_by_target"]["aarch64-apple-darwin"],
        "b".repeat(64)
    );
    assert_eq!(
        value["sha256_by_target"]["x86_64-unknown-linux-gnu"],
        "a".repeat(64)
    );
    assert_eq!(run(dir.path(), &["doctor"]).status.code(), Some(3));
}
