// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_config::{Config, ExternalConfig};
use serde_json::{Value, json};

fn workspace(external: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"version":1,"organization":{"name":"test"},"providers":[{"id":"instance","type":"external","external":external}]})).unwrap()
}
fn mapped() -> Value {
    json!({"provider":"fixture","sha256_by_target":{"aarch64-apple-darwin":"a".repeat(64),"x86_64-unknown-linux-gnu":"b".repeat(64)}})
}
#[test]
fn portable_map_parses_without_inspecting_the_native_host() {
    assert!(Config::from_bytes(&workspace(mapped())).is_ok());
    let foreign =
        json!({"provider":"fixture","sha256_by_target":{"x86_64-pc-windows-msvc":"c".repeat(64)}});
    assert!(Config::from_bytes(&workspace(foreign)).is_ok());
}
#[test]
fn scalar_serialization_retains_exact_bytes() {
    let old = format!(
        r#"{{"provider":"fixture","sha256":"{}","configuration":{{}},"credentials":{{}}}}"#,
        "a".repeat(64)
    );
    let decoded: ExternalConfig = serde_json::from_str(&old).unwrap();
    assert_eq!(serde_json::to_string(&decoded).unwrap(), old);
}
#[test]
fn pin_alternatives_are_strict_and_bounded() {
    for bad in [
        json!({"provider":"fixture"}),
        json!({"provider":"fixture","sha256":null}),
        json!({"provider":"fixture","sha256_by_target":null}),
        json!({"provider":"fixture","sha256_by_target":{}}),
        json!({"provider":"fixture","sha256_by_target":{"unknown":"a".repeat(64)}}),
        json!({"provider":"fixture","sha256_by_target":{"aarch64-apple-darwin":"A".repeat(64)}}),
        json!({"provider":"fixture","sha256_by_target":{"aarch64-apple-darwin":null}}),
        json!({"provider":"fixture","sha256_by_target":{"aarch64-apple-darwin":"bad"}}),
        json!({"provider":"fixture","sha256":"a".repeat(64),"sha256_by_target":{"aarch64-apple-darwin":"a".repeat(64)}}),
        json!({"provider":"fixture","sha256":"","sha256_by_target":{"aarch64-apple-darwin":"a".repeat(64)}}),
    ] {
        assert!(Config::from_bytes(&workspace(bad)).is_err());
    }
    // Direct serde callers and the strict workspace parser must both reject duplicate maps.
    let duplicate = format!(
        r#"{{"provider":"fixture","sha256_by_target":{{"aarch64-apple-darwin":"{}","aarch64-apple-darwin":"{}"}}}}"#,
        "a".repeat(64),
        "b".repeat(64)
    );
    assert!(serde_json::from_str::<ExternalConfig>(&duplicate).is_err());
    let yaml = format!(
        "version: 1\norganization: {{name: test}}\nproviders:\n- id: instance\n  type: external\n  external:\n    provider: fixture\n    sha256_by_target:\n      aarch64-apple-darwin: '{}'\n      aarch64-apple-darwin: '{}'\n",
        "a".repeat(64),
        "b".repeat(64)
    );
    assert!(Config::from_bytes(yaml.as_bytes()).is_err());
}

#[test]
fn strict_pin_fields_reject_duplicate_top_level_keys() {
    let scalar = format!(
        r#"{{"provider":"fixture","sha256":"{}","sha256":"{}"}}"#,
        "a".repeat(64),
        "b".repeat(64)
    );
    assert!(serde_json::from_str::<ExternalConfig>(&scalar).is_err());
    let maps = format!(
        r#"{{"provider":"fixture","sha256_by_target":{{"aarch64-apple-darwin":"{}"}},"sha256_by_target":{{"x86_64-pc-windows-msvc":"{}"}}}}"#,
        "a".repeat(64),
        "b".repeat(64)
    );
    assert!(serde_json::from_str::<ExternalConfig>(&maps).is_err());
}

#[test]
fn selection_is_exact_for_all_targets_without_registry_or_environment_fallback() {
    let targets = [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
    ];
    let pins: std::collections::BTreeMap<_, _> = targets
        .iter()
        .enumerate()
        .map(|(index, target)| ((*target).to_string(), (index + 1).to_string().repeat(64)))
        .collect();
    let config: ExternalConfig =
        serde_json::from_value(json!({"provider":"fixture","sha256_by_target":pins})).unwrap();
    for (index, target) in targets.iter().enumerate() {
        assert_eq!(
            config.digest_for_target(Some(target)).unwrap(),
            (index + 1).to_string().repeat(64)
        );
    }
    assert!(config.digest_for_target(None).is_err());
    assert!(
        config
            .digest_for_target(Some("x86_64-unknown-linux-musl"))
            .is_err()
    );
    let partial: ExternalConfig = serde_json::from_value(mapped()).unwrap();
    assert!(
        partial
            .digest_for_target(Some("x86_64-pc-windows-msvc"))
            .is_err()
    );
    let scalar: ExternalConfig =
        serde_json::from_value(json!({"provider":"fixture","sha256":"c".repeat(64)})).unwrap();
    assert_eq!(scalar.digest_for_target(None).unwrap(), "c".repeat(64));
    assert_eq!(
        scalar.digest_for_target(Some("unsupported")).unwrap(),
        "c".repeat(64)
    );
    let mut ambiguous = scalar;
    ambiguous.sha256_by_target = Some(pins);
    assert!(
        ambiguous
            .digest_for_target(Some("aarch64-apple-darwin"))
            .is_err()
    );
}

#[test]
fn direct_json_map_input_is_bounded_and_rejects_null() {
    for text in [
        r#"{"provider":"fixture","sha256_by_target":null}"#,
        r#"{"provider":"fixture","sha256":null}"#,
    ] {
        assert!(serde_json::from_str::<ExternalConfig>(text).is_err());
    }
    let oversized: std::collections::BTreeMap<_, _> = (0..6)
        .map(|index| (format!("target{index}"), "a".repeat(64)))
        .collect();
    assert!(
        serde_json::from_value::<ExternalConfig>(
            json!({"provider":"fixture","sha256_by_target":oversized})
        )
        .is_err()
    );
    let config: ExternalConfig = serde_json::from_value(mapped()).unwrap();
    let encoded = serde_json::to_value(config).unwrap();
    assert!(encoded.get("sha256").is_none());
    assert_eq!(encoded["sha256_by_target"], mapped()["sha256_by_target"]);
}
