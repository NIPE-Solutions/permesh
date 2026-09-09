// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_sdk::network::{MAX_CA_BYTES, NetworkContext};
fn context(value: serde_json::Value) -> NetworkContext {
    serde_json::from_value(value).unwrap()
}
#[test]
fn network_context_is_explicit_bounded_and_contains_no_url_credentials() {
    for proxy in [
        "http://proxy.example.com:8080",
        "https://proxy.example.com",
        "http://[::1]:3128",
    ] {
        assert!(
            context(serde_json::json!({"https_proxy":proxy}))
                .validate()
                .is_ok()
        );
    }
    for proxy in [
        "https://user:secret@proxy.example.com",
        "http://@proxy.example.com",
        "socks5://proxy.example.com",
        "http://proxy.example.com/path",
        "http://proxy.example.com/a/..",
        "http:///proxy.example.com",
        "http://proxy.example.com//",
        "http://proxy.example.com?token=secret",
        "http://proxy.example.com#secret",
        "http://proxy.example.com:0",
        " http://proxy.example.com",
        "http://proxy.example.com\n",
    ] {
        let value = context(serde_json::json!({"https_proxy":proxy}));
        assert!(value.validate().is_err(), "accepted invalid proxy");
        assert!(!format!("{value:?}").contains(proxy));
    }
    assert!(context(serde_json::json!({})).validate().is_err());
    assert!(
        context(serde_json::json!({"no_proxy":["example.com"]}))
            .validate()
            .is_err()
    );
    for bypass in [vec![".example.com", "127.0.0.1", "::1"], vec!["*"]] {
        assert!(
            context(
                serde_json::json!({"https_proxy":"http://proxy.example.com","no_proxy":bypass})
            )
            .validate()
            .is_ok()
        );
    }
    for bypass in [
        vec!["example.com", "EXAMPLE.COM"],
        vec!["example.com,private.com"],
        vec!["*.example.com"],
        vec!["https://example.com"],
        vec!["10.0.0.0/8"],
        vec!["example.com:443"],
    ] {
        assert!(
            context(
                serde_json::json!({"https_proxy":"http://proxy.example.com","no_proxy":bypass})
            )
            .validate()
            .is_err()
        );
    }
}
#[test]
fn optional_network_fields_reject_null_unknown_and_duplicate_values() {
    for text in [
        r#"{"https_proxy":null}"#,
        r#"{"ca_bundle_pem":null}"#,
        r#"{"no_proxy":null}"#,
        r#"{"proxy":"http://example.com"}"#,
        r#"{"https_proxy":"http://a","https_proxy":"http://b"}"#,
    ] {
        assert!(serde_json::from_str::<NetworkContext>(text).is_err());
    }
}
#[test]
fn only_bounded_certificate_pem_blocks_can_be_delivered() {
    let pem = "-----BEGIN CERTIFICATE-----\nMAA=\n-----END CERTIFICATE-----\n";
    assert!(
        context(serde_json::json!({"ca_bundle_pem":pem}))
            .validate()
            .is_ok()
    );
    for invalid in [
        pem.replace("CERTIFICATE", "PRIVATE KEY"),
        format!("secret\n{pem}"),
        pem.replace("MAA=", "bad=base64"),
        pem.repeat(65),
        "x".repeat(MAX_CA_BYTES + 1),
        pem.replace("MAA=", ""),
    ] {
        assert!(
            context(serde_json::json!({"ca_bundle_pem":invalid}))
                .validate()
                .is_err()
        );
    }
}
