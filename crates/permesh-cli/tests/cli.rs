// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn requested_demo_flow_and_nested_discovery() {
    let d = tempfile::tempdir().unwrap();
    assert!(run(d.path(), &["init", "--demo"]).status.success());
    std::fs::create_dir(d.path().join("nested")).unwrap();
    let doctor = run(&d.path().join("nested"), &["doctor"]);
    assert!(
        doctor.status.success(),
        "{}",
        String::from_utf8_lossy(&doctor.stderr)
    );
    let human = run(d.path(), &["user", "alice@example.com"]);
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("team/backend"));
    assert!(text.contains("Identity classification: human / active / internal"));
    assert!(text.contains("evidence: assignment"));
    assert!(text.contains("grant certainty: observed"));
    assert!(text.contains("path certainty: derived"));
    assert!(text.contains("acme/payments-api"));
    assert!(!text.contains('\x1b'));
    let json = run(d.path(), &["user", "alice@example.com", "--json"]);
    assert!(json.status.success());
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(v["schema_version"], 2);
    assert_eq!(v["result"]["identity"]["affiliation"], "internal");
    assert_eq!(v["result"]["access"][0]["certainty"], "derived");
    assert_eq!(v["result"]["access"][0]["grant"]["certainty"], "observed");
    assert_eq!(v["complete"], true);
    assert_eq!(v["result"]["access"].as_array().unwrap().len(), 2);
    assert!(json.stderr.is_empty());
}
#[test]
fn init_does_not_overwrite_and_unknown_query_has_stable_exit() {
    let d = tempfile::tempdir().unwrap();
    assert!(run(d.path(), &["init", "--demo"]).status.success());
    let original = std::fs::read(d.path().join("permesh.yaml")).unwrap();
    assert_eq!(run(d.path(), &["init", "--demo"]).status.code(), Some(2));
    assert_eq!(
        std::fs::read(d.path().join("permesh.yaml")).unwrap(),
        original
    );
    assert_eq!(
        run(d.path(), &["user", "missing", "--json"]).status.code(),
        Some(1)
    );
}
#[test]
fn plaintext_and_plugin_config_are_rejected_without_echo_or_execution() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("permesh.yaml"),"version: 1\norganization: {name: Acme}\nproviders:\n  - id: github-main\n    type: github\n    organizations: [acme]\n    auth: {token: SENTINEL_SECRET}\n").unwrap();
    let o = run(d.path(), &["doctor", "--json"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&o.stdout).contains("SENTINEL_SECRET"));
    assert!(!String::from_utf8_lossy(&o.stderr).contains("SENTINEL_SECRET"));
    std::fs::write(d.path().join("permesh.yaml"),"version: 1\norganization: {name: Acme}\nproviders:\n - id: evil\n   type: external\n   executable: ./plugins/evil\n").unwrap();
    assert_eq!(
        run(d.path(), &["user", "alice@example.com"]).status.code(),
        Some(2)
    );
    assert!(!d.path().join("executed").exists());
}
#[test]
fn partial_failure_keeps_demo_access_and_marks_incomplete() {
    let d = tempfile::tempdir().unwrap();
    run(d.path(), &["init", "--demo"]);
    let p = d.path().join("permesh.yaml");
    let mut c = permesh_config::Config::load(&p).unwrap();
    c.providers.push(permesh_config::ProviderConfig {
        id: "github-main".into(),
        kind: permesh_config::ProviderKind::Github,
        external: None,
        customer_id: None,
        organizations: vec!["acme".into()],
        auth: Some(permesh_config::AuthConfig {
            token: "env://PERMESH_TEST_DEFINITELY_MISSING".into(),
        }),
    });
    std::fs::write(p, permesh_config::to_yaml(&c).unwrap()).unwrap();
    let o = run(d.path(), &["user", "alice@example.com", "--json"]);
    assert_eq!(o.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["complete"], false);
    assert_eq!(v["schema_version"], 2);
    assert_eq!(v["result"]["access"].as_array().unwrap().len(), 2);
}
#[test]
fn json_argument_errors_never_echo_values() {
    let d = tempfile::tempdir().unwrap();
    let o = run(d.path(), &["--json", "--secret-token", "SENTINEL_SECRET"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(serde_json::from_slice::<serde_json::Value>(&o.stdout).is_ok());
    assert!(!String::from_utf8_lossy(&o.stdout).contains("SENTINEL_SECRET"));
}

#[test]
fn mixed_auth_status_is_partial_and_env_login_does_not_write() {
    let d = tempfile::tempdir().unwrap();
    assert!(run(d.path(), &["init", "--demo"]).status.success());
    let path = d.path().join("permesh.yaml");
    let mut config = permesh_config::Config::load(&path).unwrap();
    config.providers.push(permesh_config::ProviderConfig {
        id: "github-main".into(),
        kind: permesh_config::ProviderKind::Github,
        external: None,
        customer_id: None,
        organizations: vec!["acme".into()],
        auth: Some(permesh_config::AuthConfig {
            token: "env://PERMESH_TEST_DEFINITELY_MISSING".into(),
        }),
    });
    std::fs::write(&path, permesh_config::to_yaml(&config).unwrap()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let output = run(d.path(), &["auth", "status", "--json"]);
    assert_eq!(output.status.code(), Some(4));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["complete"], false);
    let login = run(
        d.path(),
        &["auth", "login", "github-main", "--token-stdin", "--json"],
    );
    assert_eq!(login.status.code(), Some(2));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn sigint_cancels_blocked_token_stdin_with_json_error() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("permesh.yaml"), "version: 1\norganization: {name: Acme}\nproviders:\n  - id: gh\n    type: external\n    external:\n      provider: github\n      sha256: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'\n      configuration: {organizations: [acme]}\n      credentials: {token: keychain://gh/token}\n").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(d.path())
        .env("TOKIO_WORKER_THREADS", "2")
        .args(["auth", "login", "gh", "--token-stdin", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Keep stdin open so the native reader cannot finish before cancellation.
    let input = child.stdin.take().unwrap();
    std::thread::sleep(Duration::from_secs(1));
    assert!(
        Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("Cancellation waited for blocked token reader");
    }
    let output = child.wait_with_output().unwrap();
    drop(input);
    assert_eq!(output.status.code(), Some(130));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["error"]["code"], 130);
    assert!(output.stderr.is_empty());
}

#[test]
fn rejected_token_input_never_reaches_output() {
    use std::{io::Write, process::Stdio};
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("permesh.yaml"), "version: 1\norganization: {name: Acme}\nproviders:\n  - id: gh\n    type: external\n    external:\n      provider: github\n      sha256: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'\n      configuration: {organizations: [acme]}\n      credentials: {token: keychain://gh/token}\n").unwrap();
    for token in [
        vec![b'x'; 16_385],
        b"SENTINEL_SECRET\x00".to_vec(),
        vec![255],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_permesh"))
            .current_dir(d.path())
            .env("TOKIO_WORKER_THREADS", "2")
            .args(["auth", "login", "gh", "--token-stdin", "--json"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&token).unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("SENTINEL_SECRET"));
        assert!(output.stderr.is_empty());
        assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
    }
}

#[test]
fn provider_add_preserves_workspace_when_yaml_expansion_exceeds_limit() {
    let directory = tempfile::tempdir().unwrap();
    let aliases: serde_json::Map<String, serde_json::Value> = (0..3930)
        .map(|n| {
            (
                format!("{n}{}", "a".repeat(120)),
                serde_json::json!({"demo":[format!("{n}{}", "b".repeat(120))]}),
            )
        })
        .collect();
    let original = serde_json::to_vec(&serde_json::json!({
        "version":1,"organization":{"name":"Example"},
        "providers":[{"id":"demo","type":"demo"}],
        "identity":{"aliases":aliases}
    }))
    .unwrap();
    permesh_config::Config::from_bytes(&original).unwrap();
    let path = directory.path().join("permesh.yaml");
    std::fs::write(&path, &original).unwrap();
    let output = run(
        directory.path(),
        &[
            "provider",
            "add",
            "external",
            "--provider",
            "fixture",
            "--sha256",
            &"a".repeat(64),
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn parsed_access_failures_use_schema_two_while_control_failures_use_one() {
    let d = tempfile::tempdir().unwrap();
    for (args, version) in [
        (vec!["user", "missing", "--json"], 2),
        (vec!["admins", "--json"], 2),
        (vec!["orphaned", "--json"], 2),
        (vec!["doctor", "--json"], 1),
    ] {
        let output = run(d.path(), &args);
        assert!(!output.status.success());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["schema_version"], version, "{args:?}");
        assert!(value["error"].is_object());
    }
}
