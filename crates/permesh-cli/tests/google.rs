// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::process::{Command, Output};
fn run(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(dir)
        .args(args)
        .env_remove("PERMESH_TEST_MISSING_GOOGLE_92741")
        .output()
        .unwrap()
}
#[test]
fn legacy_google_remains_parseable_and_does_not_hide_demo_observations() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    let path = dir.path().join("permesh.yaml");
    let mut config = permesh_config::Config::load(&path).unwrap();
    config.providers.push(permesh_config::ProviderConfig {
        id: "google-main".into(),
        kind: permesh_config::ProviderKind::Google,
        organizations: vec![],
        customer_id: Some("C12345".into()),
        external: None,
        auth: Some(permesh_config::AuthConfig {
            token: "env://PERMESH_TEST_MISSING_GOOGLE_92741".into(),
        }),
    });
    config
        .identity
        .sources
        .push(permesh_config::IdentitySource {
            provider: "google-main".into(),
            authoritative: true,
        });
    std::fs::write(&path, permesh_config::to_yaml(&config).unwrap()).unwrap();
    let path = dir.path().join("permesh.yaml");
    let config = permesh_config::Config::load(&path).unwrap();
    assert!(
        config
            .identity
            .sources
            .iter()
            .any(|s| s.provider == "google-main" && s.authoritative)
    );
    let before = std::fs::read(&path).unwrap();
    let capabilities = run(
        dir.path(),
        &["provider", "capabilities", "google-main", "--json"],
    );
    assert_eq!(capabilities.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&capabilities.stdout).contains("provider migrate google-main"));
    let partial = run(dir.path(), &["user", "alice@example.com", "--json"]);
    assert_eq!(partial.status.code(), Some(4));
    let value: serde_json::Value = serde_json::from_slice(&partial.stdout).unwrap();
    assert_eq!(value["complete"], false);
    assert!(!value["result"]["access"].as_array().unwrap().is_empty());
    assert!(partial.stderr.is_empty());
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn add_rejects_invalid_source_and_tenant_without_rewriting_config() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init"]).status.success());
    let path = dir.path().join("permesh.yaml");
    let before = std::fs::read(&path).unwrap();
    for args in [
        vec!["provider", "add", "google", "--customer-id", "my_customer"],
        vec!["provider", "add", "google"],
        vec![
            "provider",
            "add",
            "google",
            "--customer-id",
            "C123",
            "--organization",
            "ignored",
        ],
        vec![
            "provider",
            "add",
            "github",
            "--organization",
            "example",
            "--authoritative",
        ],
    ] {
        assert_eq!(run(dir.path(), &args).status.code(), Some(2));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[test]
fn adding_external_directory_does_not_implicitly_trust_identity_status() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init"]).status.success());
    assert!(
        run(
            dir.path(),
            &[
                "provider",
                "add",
                "external",
                "--id",
                "google-main",
                "--provider",
                "google",
                "--sha256",
                &"a".repeat(64),
                "--discovery-protocol",
                "negotiated-v1",
                "--setting",
                "customer_id=C12345",
                "--credential",
                "token=keychain://google-main/token"
            ]
        )
        .status
        .success()
    );
    let config = permesh_config::Config::load(&dir.path().join("permesh.yaml")).unwrap();
    assert!(config.identity.sources.is_empty());
    assert_eq!(config.providers[0].id, "google-main");
    assert_eq!(
        config.providers[0]
            .external
            .as_ref()
            .unwrap()
            .discovery_protocol,
        permesh_config::DiscoveryProtocol::NegotiatedV1
    );
    assert_eq!(
        config.providers[0].external.as_ref().unwrap().credentials["token"],
        "keychain://google-main/token"
    );
}
