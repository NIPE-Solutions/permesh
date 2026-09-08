// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_config::{Config, find_workspace, to_yaml};
const DEMO: &str = "version: 1\norganization:\n  name: Acme\nproviders:\n  - id: demo\n    type: demo\nidentity:\n  sources: [{provider: demo, authoritative: true}]\n  aliases: {}\n";
fn load(text: &str) -> permesh_config::Result<Config> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("permesh.yaml");
    std::fs::write(&path, text).unwrap();
    Config::load(&path)
}
#[test]
fn demo_roundtrips_and_discovers_nearest_workspace() {
    let cfg = load(DEMO).unwrap();
    assert!(load(&to_yaml(&cfg).unwrap()).is_ok());
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(dir.path().join("permesh.yaml"), DEMO).unwrap();
    assert_eq!(
        find_workspace(&nested).unwrap(),
        dir.path().canonicalize().unwrap().join("permesh.yaml")
    );
    std::fs::write(nested.join("permesh.yaml"), DEMO).unwrap();
    assert_eq!(
        find_workspace(&nested).unwrap(),
        nested.canonicalize().unwrap().join("permesh.yaml")
    );
}
#[test]
fn rejects_hostile_and_mistyped_yaml_without_echoing_input() {
    let bad = [
        DEMO.replace("version: 1", "version: 2"),
        format!("{DEMO}plugin: sentinel-plaintext"),
        DEMO.replace("type: demo", "type: github"),
        DEMO.replace("name: Acme", "name: !include /etc/passwd"),
        DEMO.replace("name: Acme", "name: &x Acme"),
        DEMO.replace("name: Acme", "name: Acme\n  name: sentinel-plaintext"),
        DEMO.replace("provider: demo", "provider: missing"),
        DEMO.replace(
            "  aliases: {}",
            "  aliases:\n    alice:\n      demo: [a, a]",
        ),
        DEMO.replace(
            "  aliases: {}",
            "  aliases:\n    alice: {demo: [a]}\n    bob: {demo: [a]}",
        ),
        format!("{DEMO}{}", " ".repeat(1_048_577)),
    ];
    for text in bad {
        let error = load(&text).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("sentinel-plaintext"));
    }
}
#[test]
fn rejects_plaintext_tokens_and_duplicate_providers() {
    assert!(
        load(&DEMO.replace(
            "type: demo",
            "type: github\n    organizations: [acme]\n    auth: {token: sentinel-plaintext}"
        ))
        .is_err()
    );
    assert!(load(&DEMO.replace("identity:", "  - id: demo\n    type: demo\nidentity:")).is_err());
}
#[test]
fn empty_workspace_is_valid_and_demo_rejects_ignored_organizations() {
    assert!(load("version: 1\norganization: {name: Acme}\nproviders: []\n").is_ok());
    assert!(load(&DEMO.replace("type: demo", "type: demo\n    organizations: [ignored]")).is_err());
}
#[test]
fn parse_error_reports_location_without_source() {
    let error = load(&DEMO.replace("version: 1", "version: sentinel-plaintext")).unwrap_err();
    assert!(error.to_string().contains("line 1"));
    assert!(!format!("{error:?} {error}").contains("sentinel-plaintext"));
}
#[test]
fn directories_are_not_config_files() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Config::load(dir.path()).is_err());
}
#[cfg(unix)]
#[test]
fn named_pipe_is_rejected_without_waiting_for_a_writer() {
    const NAME: &str = "PERMESH_TEST_FIFO_528432";
    if let Some(path) = std::env::var_os(NAME) {
        assert!(Config::load(std::path::Path::new(&path)).is_err());
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "named_pipe_is_rejected_without_waiting_for_a_writer",
        ])
        .env(NAME, &path)
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("configuration blocked opening a named pipe");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn google_source_is_explicit_and_rejects_wrong_provider_fields() {
    let google = "version: 1\norganization: {name: Example}\nproviders:\n  - id: directory\n    type: google\n    customer_id: C12345\n    auth: {token: env://PERMESH_GOOGLE_TOKEN}\nidentity:\n  sources: [{provider: directory, authoritative: true}]\n";
    let config = load(google).unwrap();
    assert!(load(&to_yaml(&config).unwrap()).is_ok());
    for invalid in [
        google.replace("C12345", "my_customer"),
        google.replace("C12345", "C"),
        google.replace("C12345", "https://example.com"),
        google.replace("    customer_id: C12345\n", ""),
        google.replace("type: google", "type: google\n    organizations: [ignored]"),
        google.replace("env://PERMESH_GOOGLE_TOKEN", "plaintext-sentinel"),
        google.replace("env://PERMESH_GOOGLE_TOKEN", "keychain://other/token"),
        google.replace("type: google", "type: github\n    organizations: [example]"),
    ] {
        let error = load(&invalid).unwrap_err();
        assert!(!error.to_string().contains("plaintext-sentinel"));
    }
}
