// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_sdk::browser_auth::BrowserAuthSpec;
use serde_json::{Value, json};
fn spec() -> Value {
    json!({"schema_version":1,"authorization_endpoint":"https://accounts.example.test/authorize","token_endpoint":"https://accounts.example.test/token","scopes":["directory.read"],"client_id_field":"client_id","client_secret_slot":"client_secret","refresh_token_slot":"refresh_token","when":{"field":"auth_mode","equals":"refresh_token"},"authorization_parameters":{"access_type":"offline","prompt":"consent"}})
}
#[test]
fn accepts_bounded_declaration_and_rejects_endpoint_or_oauth_override() {
    let valid: BrowserAuthSpec = serde_json::from_value(spec()).unwrap();
    assert!(valid.validate().is_ok());
    for (key, value) in [
        ("token_endpoint", json!("http://example.test/token")),
        (
            "token_endpoint",
            json!("https://user:secret@example.test/token"),
        ),
        ("token_endpoint", json!("https://example.test:8443/token")),
        (
            "token_endpoint",
            json!("https://example.test/token?client_secret=bad"),
        ),
        (
            "token_endpoint",
            json!("https://example.test/token#fragment"),
        ),
        ("authorization_endpoint", json!("javascript:alert(1)")),
        ("scopes", json!(["read write"])),
        ("authorization_parameters", json!({"scope":"admin"})),
        ("authorization_parameters", json!({"resource":"all"})),
        ("refresh_token_slot", json!("../other")),
    ] {
        let mut declaration = spec();
        declaration[key] = value;
        assert!(
            serde_json::from_value::<BrowserAuthSpec>(declaration)
                .map(|s| s.validate().is_err())
                .unwrap_or(true)
        );
    }
}
