// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::process::{Command, Output};
fn run(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .args(args)
        .env_remove("PERMESH_TEST_ORPHAN_AUTH_41975")
        .output()
        .unwrap()
}
#[test]
fn demo_orphaned_retains_inactive_access_and_separates_bot() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    let path = dir.path().join("permesh.yaml");
    let before = std::fs::read(&path).unwrap();
    let result = run(dir.path(), &["orphaned", "--json"]);
    assert_eq!(result.status.code(), Some(0));
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["command"], "orphaned");
    assert_eq!(value["complete"], true);
    assert_eq!(value["result"]["authority_complete"], true);
    let accounts = value["result"]["accounts"].as_array().unwrap();
    assert!(accounts.iter().any(|a| a["reason"] == "inactive_identity"));
    assert!(
        accounts
            .iter()
            .any(|a| a["reason"] == "bot" || a["reason"] == "service_account")
    );
    assert!(
        !accounts
            .iter()
            .any(|a| a["account"]["login"] == "alice-dev")
    );
    assert!(!value["result"]["access"].as_array().unwrap().is_empty());
    assert!(result.stderr.is_empty());
    let human = run(dir.path(), &["orphaned", "--color", "never"]);
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("Inactive identities"));
    assert!(text.contains("Service accounts") || text.contains("Bots"));
    assert!(!text.contains('\u{1b}'));
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn orphaned_requires_explicit_authority_before_discovery() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    let path = dir.path().join("permesh.yaml");
    let mut config = permesh_config::Config::load(&path).unwrap();
    config.identity.sources.clear();
    std::fs::write(&path, permesh_config::to_yaml(&config).unwrap()).unwrap();
    let output = run(dir.path(), &["orphaned", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        data["error"]["message"]
            .as_str()
            .unwrap()
            .contains("authoritative")
    );
}
#[test]
fn unavailable_authority_makes_all_accounts_unassessed() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    assert!(
        run(
            dir.path(),
            &[
                "provider",
                "add",
                "google",
                "--customer-id",
                "C123",
                "--authoritative",
                "--token-ref",
                "env://PERMESH_TEST_ORPHAN_AUTH_41975"
            ]
        )
        .status
        .success()
    );
    let output = run(dir.path(), &["orphaned", "--json"]);
    assert_eq!(output.status.code(), Some(4));
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["result"]["authority_complete"], false);
    assert!(
        data["result"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["reason"] == "unassessed")
    );
    assert_eq!(data["result"]["accounts"].as_array().unwrap().len(), 4);
    let path = dir.path().join("permesh.yaml");
    let mut config = permesh_config::Config::load(&path).unwrap();
    config.providers.retain(|p| p.id != "demo");
    config.identity.sources.retain(|s| s.provider != "demo");
    std::fs::write(path, permesh_config::to_yaml(&config).unwrap()).unwrap();
    let failed = run(dir.path(), &["orphaned", "--json"]);
    assert_eq!(failed.status.code(), Some(3));
    let data: serde_json::Value = serde_json::from_slice(&failed.stdout).unwrap();
    assert_eq!(data["result"], serde_json::json!({}));
    let failed = run(dir.path(), &["orphaned"]);
    let text = String::from_utf8(failed.stdout).unwrap();
    assert!(text.contains("could not be assessed"));
    assert!(!text.contains("0 accounts"));
}

#[test]
fn unavailable_access_provider_does_not_invalidate_complete_authority() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    assert!(
        run(
            dir.path(),
            &[
                "provider",
                "add",
                "external",
                "--provider",
                "github",
                "--sha256",
                &"a".repeat(64),
                "--credential",
                "token=env://PERMESH_TEST_ORPHAN_AUTH_41975"
            ]
        )
        .status
        .success()
    );
    let output = run(dir.path(), &["orphaned", "--json"]);
    assert_eq!(output.status.code(), Some(4));
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["complete"], false);
    assert_eq!(data["result"]["authority_complete"], true);
    assert!(
        data["result"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["reason"] == "inactive_identity")
    );
    assert!(
        data["result"]["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|a| a["reason"] != "unassessed")
    );
}
