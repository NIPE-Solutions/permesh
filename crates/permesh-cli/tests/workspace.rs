// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{
    path::Path,
    process::{Command, Output},
};
const MISSING_CREDENTIAL: &str = "PERMESH_TEST_WORKSPACE_MISSING_CREDENTIAL_728342";
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env(
            "PERMESH_DATA_DIR",
            dir.canonicalize().unwrap().join("state"),
        )
        .env("TOKIO_WORKER_THREADS", "2")
        .env_remove(MISSING_CREDENTIAL)
        .args(args)
        .output()
        .unwrap()
}
fn json(output: &Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(!output.stdout.contains(&0x1b));
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn plain_init_and_doctor_accept_an_empty_workspace() {
    let dir = tempfile::tempdir().unwrap();
    json(&run(
        dir.path(),
        &["init", "--organization", "Empty workspace", "--json"],
    ));
    let config = permesh_config::Config::load(&dir.path().join("permesh.yaml")).unwrap();
    assert!(config.providers.is_empty());
    let report = json(&run(dir.path(), &["doctor", "--json"]));
    assert_eq!(report["result"]["organization"], "Empty workspace");
    assert!(
        report["result"]["message"]
            .as_str()
            .unwrap()
            .contains("No providers configured")
    );
}
#[test]
fn provider_metadata_does_not_resolve_credentials_and_duplicate_add_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init"]).status.success());
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("inert-native");
    std::fs::write(&source, b"\x7fELFmetadata never executes this fixture").unwrap();
    let digest = permesh_provider_external::trust::inspect(&source)
        .unwrap()
        .sha256;
    let registry =
        permesh_provider_external::trust::Registry::new(root.join("state/providers")).unwrap();
    registry
        .trust(
            &source,
            "fixture",
            &digest,
            &[permesh_provider_sdk::Capability::Accounts],
        )
        .unwrap();
    let token_ref = format!("token=env://{MISSING_CREDENTIAL}");
    let args = [
        "provider",
        "add",
        "external",
        "--id",
        "external-test",
        "--provider",
        "fixture",
        "--sha256",
        &digest,
        "--credential",
        &token_ref,
        "--json",
    ];
    json(&run(dir.path(), &args));
    let path = dir.path().join("permesh.yaml");
    let config = permesh_config::Config::load(&path).unwrap();
    assert_eq!(
        config.providers[0].external.as_ref().unwrap().credentials["token"],
        format!("env://{MISSING_CREDENTIAL}")
    );
    json(&run(dir.path(), &["provider", "list", "--json"]));
    let capabilities = json(&run(
        dir.path(),
        &["provider", "capabilities", "external-test", "--json"],
    ));
    assert_eq!(capabilities["result"]["metadata"]["kind"], "external");
    let before = std::fs::read(&path).unwrap();
    let duplicate = run(dir.path(), &args);
    assert_eq!(duplicate.status.code(), Some(2));
    assert!(
        serde_json::from_slice::<serde_json::Value>(&duplicate.stdout).unwrap()["error"]
            .is_object()
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn explicit_config_wins_over_nested_workspace_discovery() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        run(
            dir.path(),
            &["init", "--demo", "--organization", "Selected workspace"]
        )
        .status
        .success()
    );
    let nested = dir.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    assert!(
        run(&nested, &["init", "--organization", "Nested workspace"])
            .status
            .success()
    );
    let report = json(&run(
        &nested,
        &["--config", "../permesh.yaml", "doctor", "--json"],
    ));
    assert_eq!(report["result"]["organization"], "Selected workspace");
    let query = json(&run(
        &nested,
        &[
            "--config",
            "../permesh.yaml",
            "user",
            "alice@example.com",
            "--json",
        ],
    ));
    assert_eq!(query["result"]["access"].as_array().unwrap().len(), 2);
    let local = json(&run(&nested, &["doctor", "--json"]));
    assert_eq!(local["result"]["organization"], "Nested workspace");
}
#[test]
fn alias_with_missing_native_account_is_known_but_never_matches_login() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    let path = dir.path().join("permesh.yaml");
    let mut config = permesh_config::Config::load(&path).unwrap();
    let provider = config.providers[0].id.clone();
    config.identity.aliases.insert(
        "configured-person".into(),
        [(provider, vec!["alice-dev".into()])].into(),
    );
    std::fs::write(path, permesh_config::to_yaml(&config).unwrap()).unwrap();
    let report = json(&run(dir.path(), &["user", "configured-person", "--json"]));
    assert_eq!(report["result"]["identity"]["id"], "configured-person");
    assert_eq!(report["result"]["identity"]["status"], "unknown");
    assert!(report["result"]["accounts"].as_array().unwrap().is_empty());
    assert!(report["result"]["access"].as_array().unwrap().is_empty());
}
#[test]
fn help_version_and_json_work_without_a_workspace_or_terminal() {
    let dir = tempfile::tempdir().unwrap();
    for args in [&["--help"][..], &["--version"][..]] {
        let output = run(dir.path(), args);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.contains(&0x1b));
        assert!(String::from_utf8_lossy(&output.stdout).contains("permesh"));
    }
    let help = run(dir.path(), &["--help"]);
    assert!(String::from_utf8_lossy(&help.stdout).contains("--json"));
    let version = json(&run(dir.path(), &["version", "--json"]));
    assert_eq!(version["schema_version"], 1);
    assert_eq!(version["result"]["telemetry"], false);
    assert_eq!(version["result"]["backend"], false);
    assert!(version["result"]["version"].is_string());
}
#[cfg(unix)]
#[test]
fn executable_workspace_plugin_is_never_run_by_demo_queries() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    let plugins = dir.path().join("plugins");
    std::fs::create_dir(&plugins).unwrap();
    let script = plugins.join("evil");
    std::fs::write(&script, "#!/bin/sh\nprintf executed > plugin-executed\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let report = json(&run(dir.path(), &["user", "alice@example.com", "--json"]));
    assert_eq!(report["result"]["access"].as_array().unwrap().len(), 2);
    assert!(!dir.path().join("plugin-executed").exists());
    assert!(!plugins.join("plugin-executed").exists());
}
#[cfg(unix)]
#[test]
fn init_refuses_symlinks_without_changing_target_or_link() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("important.yaml");
    std::fs::write(&target, "original target content").unwrap();
    let path = dir.path().join("permesh.yaml");
    symlink(&target, &path).unwrap();
    let output = run(dir.path(), &["init", "--demo", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "original target content"
    );
    assert_eq!(std::fs::read_link(path).unwrap(), target);
}
