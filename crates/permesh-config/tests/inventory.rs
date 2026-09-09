// SPDX-License-Identifier: MIT
#![allow(clippy::expect_used)]
use permesh_config::Config;
use serde_json::{Value, json};
fn workspace(provider: Value) -> Value {
    json!({"version":1,"organization":{"name":"Test"},"providers":[provider],"identity":{"sources":[{"provider":"roster","authoritative":true}]}})
}
fn provider() -> Value {
    json!({"id":"roster","type":"inventory","inventory":{"path":"data/roster.json","sha256":"a".repeat(64)}})
}
fn parse(value: &Value) -> Result<Config, permesh_config::Error> {
    Config::from_bytes(value.to_string().as_bytes())
}
#[test]
fn inventory_is_explicit_syntax_only_and_defaults_to_one_day() {
    let config = parse(&workspace(provider()))
        .expect("explicit local inventory should parse without reading its nonexistent file");
    let encoded = serde_json::to_value(config).expect("serialize");
    assert_eq!(
        encoded["providers"][0]["inventory"]["max_age_seconds"],
        86400
    );
}
#[test]
fn invalid_paths_digests_options_and_wrong_provider_kinds_reject() {
    for path in [
        "/absolute.json",
        "../outside.json",
        "data/../roster.json",
        "C:/roster.json",
        "data\\roster.json",
        "",
        "./roster.json",
    ] {
        let mut p = provider();
        p["inventory"]["path"] = json!(path);
        assert!(parse(&workspace(p)).is_err(), "{path}");
    }
    for digest in ["a".repeat(63), "G".repeat(64), "A".repeat(64)] {
        let mut p = provider();
        p["inventory"]["sha256"] = json!(digest);
        assert!(parse(&workspace(p)).is_err());
    }
    for age in [0, 604801] {
        let mut p = provider();
        p["inventory"]["max_age_seconds"] = json!(age);
        assert!(parse(&workspace(p)).is_err());
    }
    for field in ["inventory", "path", "sha256", "max_age_seconds"] {
        let mut p = provider();
        if field == "inventory" {
            p[field] = Value::Null;
        } else {
            p["inventory"][field] = Value::Null;
        }
        assert!(parse(&workspace(p)).is_err());
    }
    for kind in ["demo", "github", "google", "external"] {
        let mut p = provider();
        p["type"] = json!(kind);
        assert!(parse(&workspace(p)).is_err());
    }
    for field in ["auth", "organizations", "customer_id"] {
        let mut p = provider();
        p[field] = match field {
            "auth" => json!({"token":"env:SECRET"}),
            "organizations" => json!(["acme"]),
            _ => json!("C123"),
        };
        assert!(parse(&workspace(p)).is_err());
    }
}
#[test]
fn legacy_provider_serialization_remains_exact() {
    let mut p = provider();
    p["type"] = json!("demo");
    p.as_object_mut().expect("object").remove("inventory");
    let config = parse(&workspace(p)).expect("legacy demo");
    assert_eq!(
        serde_json::to_string(&config.providers[0]).expect("serialize"),
        r#"{"id":"roster","type":"demo","organizations":[]}"#
    );
}

#[test]
fn inventory_duplicate_fields_and_missing_declarations_reject_in_json_and_yaml() {
    let json = r#"{"version":1,"organization":{"name":"T"},"providers":[{"id":"r","type":"inventory","inventory":{"path":"r.json","path":"s.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}]}"#;
    assert!(Config::from_bytes(json.as_bytes()).is_err());
    let yaml = "version: 1\norganization: {name: T}\nproviders:\n  - id: r\n    type: inventory\n    inventory:\n      path: r.json\n      path: s.json\n      sha256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
    assert!(Config::from_bytes(yaml.as_bytes()).is_err());
    let mut p = provider();
    p.as_object_mut().expect("object").remove("inventory");
    assert!(parse(&workspace(p)).is_err());
}
