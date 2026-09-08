// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use std::{
    path::Path,
    process::{Command, Output},
};
fn run(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_permesh"))
        .current_dir(path)
        .env("NO_COLOR", "1")
        .env_remove("PERMESH_ADMINS_MISSING_TOKEN")
        .args(args)
        .output()
        .unwrap()
}
fn demo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(dir.path(), &["init", "--demo"]).status.success());
    dir
}
#[test]
fn admins_reports_bob_and_preserves_role_and_provenance_in_json() {
    let d = demo();
    let original = std::fs::read(d.path().join("permesh.yaml")).unwrap();
    let output = run(d.path(), &["admins", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["schema_version"], 2);
    assert_eq!(v["command"], "admins");
    assert_eq!(v["complete"], true);
    assert_eq!(v["result"]["access"].as_array().unwrap().len(), 1);
    assert_eq!(v["result"]["access"][0]["grant"]["role"], "Admin");
    assert_eq!(v["result"]["access"][0]["grant"]["privilege"], "admin");
    assert_eq!(
        v["result"]["access"][0]["grant"]["provenance"]["method"],
        "synthetic demo fixture"
    );
    assert_eq!(v["result"]["accounts"][0]["account"]["login"], "bob-admin");
    assert_eq!(v["result"]["accounts"][0]["identity"]["state"], "resolved");
    assert_eq!(
        v["result"]["accounts"][0]["identity"]["identity"]["id"],
        "bob@example.com"
    );
    assert!(v["result"]["unknown_access"].as_array().unwrap().is_empty());
    assert!(
        v["result"]["unresolved_grants"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        std::fs::read(d.path().join("permesh.yaml")).unwrap(),
        original
    );
}
#[test]
fn human_admins_is_clear_in_non_tty_output() {
    let d = demo();
    let o = run(d.path(), &["admins"]);
    assert!(o.status.success());
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.contains("Privileged access"));
    assert!(text.contains("bob-admin"));
    assert!(text.contains("bob@example.com"));
    assert!(text.contains("acme/infrastructure"));
    assert!(text.contains("Admin (admin)"));
    assert!(!text.contains("alice-dev"));
    assert!(!text.contains('\x1b'));
}
#[test]
fn failed_provider_does_not_hide_known_admin_and_all_failed_is_not_empty_success() {
    let d = demo();
    let path = d.path().join("permesh.yaml");
    let mut c = permesh_config::Config::load(&path).unwrap();
    c.providers.push(permesh_config::ProviderConfig {
        id: "github-main".into(),
        kind: permesh_config::ProviderKind::Github,
        external: None,
        customer_id: None,
        organizations: vec!["example".into()],
        auth: Some(permesh_config::AuthConfig {
            token: "env://PERMESH_ADMINS_MISSING_TOKEN".into(),
        }),
    });
    std::fs::write(&path, permesh_config::to_yaml(&c).unwrap()).unwrap();
    let o = run(d.path(), &["admins", "--json"]);
    assert_eq!(o.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["complete"], false);
    assert_eq!(v["result"]["access"].as_array().unwrap().len(), 1);
    c.providers.remove(0);
    c.identity.sources.clear();
    std::fs::write(&path, permesh_config::to_yaml(&c).unwrap()).unwrap();
    let o = run(d.path(), &["admins", "--json"]);
    assert_eq!(o.status.code(), Some(3));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["complete"], false);
    assert_eq!(v["result"], serde_json::json!({}));
    let o = run(d.path(), &["admins"]);
    let text = String::from_utf8(o.stdout).unwrap();
    assert!(text.contains("unavailable"));
    assert!(!text.contains("0 privileged"));
}
#[test]
fn admins_requires_configured_providers_but_is_discoverable() {
    let d = tempfile::tempdir().unwrap();
    assert!(run(d.path(), &["init"]).status.success());
    assert_eq!(run(d.path(), &["admins", "--json"]).status.code(), Some(2));
    let o = run(d.path(), &["admins", "--help"]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("privileged"));
}
#[test]
fn ambiguous_identity_remains_visible_in_admins_report() {
    let d = demo();
    let path = d.path().join("permesh.yaml");
    let mut config = permesh_config::Config::load(&path).unwrap();
    config.identity.aliases.insert(
        "another@example.com".into(),
        [("demo".into(), vec!["101".into()])].into(),
    );
    std::fs::write(&path, permesh_config::to_yaml(&config).unwrap()).unwrap();
    let output = run(d.path(), &["admins", "--json"]);
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["result"]["access"].as_array().unwrap().len(), 1);
    assert_eq!(v["result"]["accounts"][0]["identity"]["state"], "ambiguous");
    assert_eq!(
        v["result"]["accounts"][0]["identity"]["candidates"],
        serde_json::json!(["another@example.com", "bob@example.com"])
    );
    let output = run(d.path(), &["admins"]);
    assert!(String::from_utf8_lossy(&output.stdout).contains("ambiguous"));
}
