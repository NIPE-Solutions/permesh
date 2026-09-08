// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_config::{Config, to_yaml};
fn source() -> String {
    format!(
        "version: 1\norganization: {{name: Example}}\nproviders:\n  - id: internal-main\n    type: external\n    external:\n      provider: fixture\n      sha256: '{}'\n      configuration: {{endpoint: 'https://service.example.com', regions: [a, b]}}\n      credentials: {{token: 'env://PERMESH_TOKEN', client_secret: 'keychain://internal-main/client_secret'}}\nidentity:\n  sources: [{{provider: internal-main, authoritative: true}}]\n",
        "a".repeat(64)
    )
}
fn load(source: &str) -> permesh_config::Result<Config> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("permesh.yaml");
    std::fs::write(&path, source).unwrap();
    Config::load(&path)
}
#[test]
fn external_parameters_and_named_references_roundtrip_without_execution() {
    let config = load(&source()).unwrap();
    assert!(load(&to_yaml(&config).unwrap()).is_ok());
}
#[test]
fn external_rejects_plaintext_unsafe_references_and_mixed_fields() {
    let good = source();
    for invalid in [
        good.replace("env://PERMESH_TOKEN", "sentinel-plaintext"),
        good.replace("env://PERMESH_TOKEN", "exec://secret"),
        good.replace(
            "keychain://internal-main/client_secret",
            "keychain://other/client_secret",
        ),
        good.replace(
            "keychain://internal-main/client_secret",
            "keychain://internal-main/token",
        ),
        good.replace("provider: fixture", "provider: ../fixture"),
        good.replace(&"a".repeat(64), "bad-digest"),
        good.replace(
            "type: external",
            "type: external\n    organizations: [example]",
        ),
        good.replace(
            "type: external",
            "type: external\n    auth: {token: env://PERMESH_TOKEN}",
        ),
        good.replace(
            "provider: fixture",
            "provider: fixture\n      executable: ./malicious",
        ),
        good.replace("credentials: {token:", "credentials: {token/bad:"),
    ] {
        let error = load(&invalid).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("sentinel-plaintext"));
    }
}
#[test]
fn external_config_is_bounded_and_disallowed_on_builtin_providers() {
    let good = source();
    assert!(load(&good.replace("https://service.example.com", &"x".repeat(65537))).is_err());
    assert!(load(&good.replace("type: external", "type: demo")).is_err());
    let mut nested = "1".to_string();
    for _ in 0..20 {
        nested = format!("[{nested}]");
    }
    assert!(load(&good.replace("[a, b]", &nested)).is_err());
}

#[test]
fn discovery_protocol_is_explicit_and_legacy_serialization_is_unchanged() {
    let legacy = load(&source()).unwrap();
    let implicit = serde_json::to_value(&legacy).unwrap();
    let explicit = load(&source().replace(
        "provider: fixture",
        "provider: fixture\n      discovery_protocol: legacy",
    ))
    .unwrap();
    assert_eq!(implicit, serde_json::to_value(explicit).unwrap());
    assert!(
        implicit["providers"][0]["external"]
            .get("discovery_protocol")
            .is_none()
    );
    let negotiated = load(&source().replace(
        "provider: fixture",
        "provider: fixture\n      discovery_protocol: negotiated_v1",
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_value(&negotiated).unwrap()["providers"][0]["external"]["discovery_protocol"],
        "negotiated_v1"
    );
    assert!(
        to_yaml(&negotiated)
            .unwrap()
            .contains("discovery_protocol: negotiated_v1")
    );
    for unsupported in ["auto", "5", "negotiated_v6", "legacy_v2"] {
        assert!(
            load(&source().replace(
                "provider: fixture",
                &format!("provider: fixture\n      discovery_protocol: {unsupported}")
            ))
            .is_err()
        );
    }
}
