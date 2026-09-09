// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_config::Config;
use serde_json::{Value, json};
fn config() -> Value {
    json!({"version":1,"organization":{"name":"Test"},"providers":[{"id":"access","type":"external","external":{"provider":"github","sha256":"a".repeat(64),"credentials":{"token":"vault://api-token"},"credential_resolvers":{"api-token":{"version":1,"type":"vault_kv2","origin":"https://vault.example.test","mount":"secret","path":"providers/github","field":"token","bootstrap":"env://BOOTSTRAP"}}}}]})
}
fn parse(value: &Value) -> Result<Config, permesh_config::Error> {
    Config::from_bytes(value.to_string().as_bytes())
}
#[test]
fn resolver_declarations_parse_without_reading_bootstrap_or_contacting_endpoint() {
    assert!(parse(&config()).is_ok());
}
#[test]
fn unsafe_origins_locators_recursive_bootstrap_and_unbound_entries_fail() {
    for origin in [
        "http://vault.example.test",
        "https://user:SECRET@vault.example.test",
        "https://vault.example.test/path",
        "https://vault.example.test?x",
        "https://vault.example.test#x",
        "https://vault.example.test\\evil",
        "https://vault.example.test/../",
    ] {
        let mut c = config();
        c["providers"][0]["external"]["credential_resolvers"]["api-token"]["origin"] =
            json!(origin);
        assert!(parse(&c).is_err(), "{origin}");
    }
    for path in ["../x", "x//y", "/x", "x%2fy", "x\\y", "x/./y", "x?secret"] {
        let mut c = config();
        c["providers"][0]["external"]["credential_resolvers"]["api-token"]["path"] = json!(path);
        assert!(parse(&c).is_err());
    }
    for bootstrap in [
        "vault://another",
        "1password://nested",
        "keychain://other/bootstrap",
        "plaintext",
    ] {
        let mut c = config();
        c["providers"][0]["external"]["credential_resolvers"]["api-token"]["bootstrap"] =
            json!(bootstrap);
        assert!(parse(&c).is_err());
    }
    let mut c = config();
    c["providers"][0]["external"]["credentials"]["token"] = json!("openbao://api-token");
    assert!(parse(&c).is_err());
    let mut c = config();
    c["providers"][0]["external"]["credentials"] = json!({});
    assert!(parse(&c).is_err());
    let mut c = config();
    c["providers"][0]["external"]["credential_resolvers"] = json!({});
    assert!(parse(&c).is_err());
    for field in ["version", "network", "secret_version"] {
        let mut c = config();
        c["providers"][0]["external"]["credential_resolvers"]["api-token"][field] = Value::Null;
        assert!(parse(&c).is_err());
    }
}
#[test]
fn existing_external_serialization_does_not_gain_empty_resolver_fields() {
    let mut c = config();
    c["providers"][0]["external"]["credentials"] = json!({"token":"env://TOKEN"});
    c["providers"][0]["external"]
        .as_object_mut()
        .unwrap()
        .remove("credential_resolvers");
    let c = parse(&c).unwrap();
    assert!(
        !serde_json::to_string(&c)
            .unwrap()
            .contains("credential_resolvers")
    );
}
#[test]
fn backend_names_are_distinct_and_locators_encode_once() {
    for (scheme, backend) in [("vault", "vault_kv2"), ("openbao", "openbao_kv2")] {
        let mut c = config();
        c["providers"][0]["external"]["credentials"]["token"] =
            json!(format!("{scheme}://api-token"));
        let r = &mut c["providers"][0]["external"]["credential_resolvers"]["api-token"];
        r["type"] = json!(backend);
        r["path"] = json!("team one/a+b");
        r["secret_version"] = json!(7);
        r["bootstrap"] = json!("keychain://access/independent-bootstrap-account");
        let parsed = parse(&c).unwrap();
        let resolver = &parsed.providers[0]
            .external
            .as_ref()
            .unwrap()
            .credential_resolvers["api-token"];
        assert_eq!(
            resolver.target_url().unwrap().as_str(),
            "https://vault.example.test/v1/secret/data/team%20one/a+b?version=7"
        );
    }
    let mut c = config();
    c["providers"][0]["external"]["credentials"]["token"] = json!("1password://api-token");
    c["providers"][0]["external"]["credential_resolvers"]["api-token"] = json!({"type":"1password_connect","version":1,"origin":"https://connect.example.test:8443/","vault":"abcdefghijklmnopqrstuvwxyz","item":"abcdefghijklmnopqrstuvwxyz","field":"custom-field-id","bootstrap":"env://BOOTSTRAP"});
    assert!(parse(&c).is_ok());
}
#[test]
fn unknown_duplicate_and_null_entries_fail_without_reflection() {
    for (key, value) in [
        ("typo", json!("PRIVATE_SENTINEL")),
        ("secret_version", json!(0)),
        ("version", json!(2)),
    ] {
        let mut c = config();
        c["providers"][0]["external"]["credential_resolvers"]["api-token"][key] = value;
        let error = parse(&c).unwrap_err();
        assert!(!error.to_string().contains("PRIVATE_SENTINEL"));
    }
    let c = config();
    let entry = c["providers"][0]["external"]["credential_resolvers"]["api-token"].to_string();
    let duplicated = c.to_string().replace(
        &format!("\"api-token\":{entry}"),
        &format!("\"api-token\":{entry},\"api-token\":{entry}"),
    );
    assert!(Config::from_bytes(duplicated.as_bytes()).is_err());
    let mut c = config();
    c["providers"][0]["external"]["credential_resolvers"] = Value::Null;
    assert!(parse(&c).is_err());
    let mut resolver = serde_json::to_value(permesh_config::CredentialResolver::VaultKv2 {
        version: 1,
        origin: "https://user:PRIVATE_SENTINEL@vault.example".into(),
        mount: "secret".into(),
        path: "one".into(),
        field: "token".into(),
        bootstrap: "env://BOOTSTRAP".into(),
        secret_version: None,
        network: None,
    })
    .unwrap();
    let invalid: permesh_config::CredentialResolver =
        serde_json::from_value(resolver.take()).unwrap();
    assert!(!format!("{invalid:?}").contains("PRIVATE_SENTINEL"));
}

#[test]
fn same_named_resolver_in_another_instance_cannot_supply_a_missing_declaration() {
    let mut c = config();
    let mut other = c["providers"][0].clone();
    other["id"] = json!("second");
    other["external"]["credential_resolvers"] = json!({});
    c["providers"].as_array_mut().unwrap().push(other);
    assert!(parse(&c).is_err());
}

#[test]
fn yaml_duplicate_resolver_names_are_rejected() {
    let entry =
        config()["providers"][0]["external"]["credential_resolvers"]["api-token"].to_string();
    let yaml = format!(
        "version: 1\norganization: {{name: test}}\nproviders:\n  - id: access\n    type: external\n    external:\n      provider: github\n      sha256: {}\n      credentials: {{token: 'vault://api-token'}}\n      credential_resolvers:\n        api-token: {entry}\n        api-token: {entry}\n",
        "a".repeat(64)
    );
    assert!(Config::from_bytes(yaml.as_bytes()).is_err());
}
