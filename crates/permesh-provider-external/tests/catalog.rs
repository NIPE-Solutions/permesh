// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_external::catalog::{TARGETS, native_target, parse, validate_request};
use serde_json::{Value, json};

fn release(version: &str) -> Value {
    json!({"provider":"example","version":version,"target":"x86_64-unknown-linux-gnu","capabilities":["accounts"],"protocols":[2,3],"archive_sha256":"a".repeat(64),"executable_sha256":"b".repeat(64),"archive_size":42})
}
fn encoded(releases: Vec<Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({"schema_version":1,"releases":releases})).unwrap()
}

#[test]
fn request_validation_and_native_target_match_catalog_contract() {
    assert!(parse(&encoded(vec![])).unwrap().releases.is_empty());
    for provider in ["example", "provider-2", "provider_2"] {
        assert!(validate_request(provider, None).is_ok());
        assert!(validate_request(provider, Some("1.2.3")).is_ok());
    }
    for provider in ["", "Uppercase", "../escape", "1first", "provider/path"] {
        assert!(validate_request(provider, None).is_err());
    }
    assert!(validate_request(&"a".repeat(33), None).is_err());
    assert!(validate_request(&"a".repeat(32), None).is_ok());
    for version in [
        "v1.0.0",
        "01.0.0",
        "1.0",
        "1.0.0-rc",
        "1.0.0+build",
        "=1.0.0",
    ] {
        assert!(validate_request("example", Some(version)).is_err());
    }
    if let Some(target) = native_target() {
        assert!(TARGETS.contains(&target));
    }
}
#[test]
fn selects_exact_or_latest_numeric_stable_release_and_derives_asset_url() {
    let catalog = parse(&encoded(vec![release("1.9.0"), release("1.10.0")])).unwrap();
    let selected = catalog
        .select("example", "x86_64-unknown-linux-gnu", None)
        .unwrap();
    assert_eq!(selected.version.to_string(), "1.10.0");
    assert_eq!(
        selected.asset_url().unwrap(),
        "https://github.com/NIPE-Solutions/permesh-providers/releases/download/example-v1.10.0/permesh-provider-example-1.10.0-x86_64-unknown-linux-gnu.zip"
    );
    assert_eq!(
        catalog
            .select("example", "x86_64-unknown-linux-gnu", Some("1.9.0"))
            .unwrap()
            .version
            .to_string(),
        "1.9.0"
    );
    for version in ["latest", "^1.0", "1.0", "1.0.0-pre", "1.0.0+build"] {
        assert!(
            catalog
                .select("example", "x86_64-unknown-linux-gnu", Some(version))
                .is_err()
        );
    }
    assert!(
        catalog
            .select("other", "x86_64-unknown-linux-gnu", None)
            .is_err()
    );
}
#[test]
fn rejects_ambiguous_or_unbounded_catalogs() {
    assert!(parse(br#"{"schema_version":1,"schema_version":1,"releases":[]}"#).is_err());
    assert!(parse(br#"{"schema_version":1,"releases":[],"unknown":true}"#).is_err());
    assert!(parse(&vec![b' '; 1024 * 1024 + 1]).is_err());
    assert!(parse(&encoded(vec![release("1.0.0"); 4097])).is_err());
    assert!(parse(&encoded(vec![release("1.0.0"); 2])).is_err());
    let valid = String::from_utf8(encoded(vec![release("1.0.0")])).unwrap();
    assert!(
        parse(
            valid
                .replace(
                    "\"provider\":\"example\"",
                    "\"provider\":\"example\",\"provider\":\"example\""
                )
                .as_bytes()
        )
        .is_err()
    );
    for (key, value) in [
        ("provider", json!("../evil")),
        ("provider", json!("Example")),
        ("version", json!("1.0.0+build")),
        ("version", json!("1.0.0-rc.1")),
        ("target", json!("arbitrary")),
        ("protocols", json!([3])),
        ("protocols", json!([2, 2])),
        ("protocols", json!([2, 4])),
        ("capabilities", json!(["accounts", "accounts"])),
        ("archive_sha256", json!("A".repeat(64))),
        ("archive_size", json!(134217729)),
        ("archive_size", json!(0)),
        ("unknown", json!(true)),
    ] {
        let mut item = release("1.0.0");
        item[key] = value;
        assert!(parse(&encoded(vec![item])).is_err(), "accepted {key}");
    }
}

#[test]
fn negotiated_discovery_requires_an_explicit_consistent_contract() {
    let mut item = release("1.0.0");
    item["discovery_protocol"] = json!("negotiated_v1");
    item["protocols"] = json!([3]);
    let catalog = parse(&encoded(vec![item.clone()])).unwrap();
    assert_eq!(serde_json::to_value(&catalog.releases[0]).unwrap(), item);
    for protocols in [
        json!([]),
        json!([2]),
        json!([2, 3]),
        json!([3, 3]),
        json!([3, 4]),
    ] {
        let mut invalid = item.clone();
        invalid["protocols"] = protocols;
        assert!(parse(&encoded(vec![invalid])).is_err());
    }
    for selector in [
        Value::Null,
        json!("unknown"),
        json!("negotiated-v1"),
        json!(3),
        json!({"negotiated_v1": null}),
    ] {
        let mut invalid = item.clone();
        invalid["discovery_protocol"] = selector;
        // One unknown release invalidates the catalog, even with a valid match.
        assert!(parse(&encoded(vec![release("0.9.0"), invalid])).is_err());
    }
    for protocols in [json!([2]), json!([2, 3]), json!([3, 2])] {
        let mut legacy = release("0.9.0");
        legacy["protocols"] = protocols;
        assert!(parse(&encoded(vec![legacy.clone(), item.clone()])).is_ok());
        legacy["discovery_protocol"] = json!("legacy");
        assert!(parse(&encoded(vec![legacy, item.clone()])).is_ok());
    }
    let bytes = String::from_utf8(encoded(vec![item])).unwrap();
    assert!(parse(bytes.replace("\"discovery_protocol\":\"negotiated_v1\"", "\"discovery_protocol\":\"negotiated_v1\",\"discovery_protocol\":\"negotiated_v1\"").as_bytes()).is_err());
}

#[test]
fn legacy_release_serialization_preserves_persisted_bytes() {
    let bytes = format!(
        r#"{{"provider":"example","version":"1.0.0","target":"x86_64-unknown-linux-gnu","capabilities":["accounts"],"protocols":[2,3],"archive_sha256":"{}","executable_sha256":"{}","archive_size":42}}"#,
        "a".repeat(64),
        "b".repeat(64)
    );
    let release: permesh_provider_external::catalog::Release =
        serde_json::from_str(&bytes).unwrap();
    release.validate().unwrap();
    assert_eq!(serde_json::to_string(&release).unwrap(), bytes);
}
