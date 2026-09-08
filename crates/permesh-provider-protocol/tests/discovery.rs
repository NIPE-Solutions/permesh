// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_protocol::{ProtocolError, handshake_request, validate_discovery};
use serde_json::{Value, json};
use std::io::Cursor;

fn exchange() -> Vec<Value> {
    vec![
        json!({"protocol":1,"id":"handshake","event":"handshake","provider":"test-provider","capabilities":["accounts","resources","grants"],"draft":true}),
        json!({"protocol":1,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"test-main","id":"alice"},"login":"alice","kind":"human","verified_emails":[]}}),
        json!({"protocol":1,"id":"discover","event":"record","kind":"resource","data":{"key":{"provider":"test-main","id":"repo"},"name":"Test repository"}}),
        json!({"protocol":1,"id":"discover","event":"record","kind":"grant","data":{"id":"grant","subject":{"kind":"account","key":{"provider":"test-main","id":"alice"}},"resource":{"provider":"test-main","id":"repo"},"role":"read","privilege":"standard","certainty":"observed","provenance":{"method":"synthetic","observed_at":"2026-01-01T00:00:00Z"}}}),
        json!({"protocol":1,"id":"discover","event":"complete","count":3,"complete":true,"limitations":[]}),
    ]
}
fn bytes(events: &[Value]) -> Vec<u8> {
    events
        .iter()
        .flat_map(|v| {
            let mut b = serde_json::to_vec(v).unwrap();
            b.push(b'\n');
            b
        })
        .collect()
}
fn validate(events: &[Value]) -> Result<permesh_core::Snapshot, ProtocolError> {
    validate_discovery(Cursor::new(bytes(events)), "test-provider", "test-main")
}
fn rejected(events: &[Value], expected: ProtocolError) {
    let error = validate(events).unwrap_err();
    assert_eq!(error, expected);
    assert!(!format!("{error:?} {error}").contains("SENTINEL"));
}
#[test]
fn valid_discovery_preserves_normalized_access_and_deterministic_order() {
    let mut events = exchange();
    let first = validate(&events).unwrap();
    assert!(first.complete);
    assert_eq!(first.accounts[0].key.id, "alice");
    assert_eq!(first.grants[0].provenance.method, "synthetic");
    events.swap(1, 3); // Forward references are allowed within a bounded snapshot.
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(validate(&events).unwrap()).unwrap()
    );
}
#[test]
fn rejects_version_identity_sequence_and_completion_violations() {
    let original = exchange();
    for (index, field, value, error) in [
        (0, "protocol", json!(2), ProtocolError::Version),
        (
            0,
            "provider",
            json!("SENTINEL-wrong"),
            ProtocolError::Provider,
        ),
        (0, "id", json!("SENTINEL-id"), ProtocolError::Sequence),
        (1, "id", json!("handshake"), ProtocolError::Sequence),
        (4, "count", json!(2), ProtocolError::Count),
        (0, "draft", json!(false), ProtocolError::Version),
    ] {
        let mut events = original.clone();
        events[index][field] = value;
        rejected(&events, error);
    }
    rejected(&original[1..], ProtocolError::Sequence);
    rejected(&original[..4], ProtocolError::Incomplete);
    let mut extra = original.clone();
    extra.push(original[4].clone());
    rejected(&extra, ProtocolError::Sequence);
    let mut early = original.clone();
    early.insert(2, original[0].clone());
    rejected(&early, ProtocolError::Sequence);
}
#[test]
fn requires_negotiated_capabilities_and_rejects_unknown_fields_everywhere() {
    let mut events = exchange();
    events[0]["capabilities"] = json!(["accounts", "resources"]);
    rejected(&events, ProtocolError::Capability);
    let mut events = exchange();
    events[0]["capabilities"] = json!(["accounts", "accounts"]);
    rejected(&events, ProtocolError::Capability);
    let mut events = exchange();
    events[1]["extra"] = json!("SENTINEL");
    rejected(&events, ProtocolError::Schema);
    let mut events = exchange();
    events[1]["data"]["key"]["extra"] = json!("SENTINEL");
    rejected(&events, ProtocolError::Schema);
    let mut events = exchange();
    events[3]["data"]["subject"]["extra"] = json!("SENTINEL");
    rejected(&events, ProtocolError::Schema);
    let mut events = exchange();
    events[3]["data"]["provenance"]["extra"] = json!("SENTINEL");
    rejected(&events, ProtocolError::Schema);
    let mut events = exchange();
    events[3]["data"]["privilege"] = json!("super-admin");
    rejected(&events, ProtocolError::Schema);
}
#[test]
fn snapshot_validation_rejects_duplicate_dangling_cross_instance_and_invalid_provenance() {
    let mut events = exchange();
    events[1]["data"]["key"]["provider"] = json!("other");
    rejected(&events, ProtocolError::Snapshot);
    let mut events = exchange();
    events[3]["data"]["resource"]["id"] = json!("missing");
    rejected(&events, ProtocolError::Snapshot);
    let mut events = exchange();
    events[3]["data"]["provenance"]["observed_at"] = json!("yesterday");
    rejected(&events, ProtocolError::Snapshot);
    let mut events = exchange();
    events.insert(2, events[1].clone());
    events[5]["count"] = json!(4);
    rejected(&events, ProtocolError::Snapshot);
}
#[test]
fn only_explicit_partial_completion_retains_a_valid_partial_snapshot() {
    let mut events = exchange();
    events[4]["complete"] = json!(false);
    events[4]["limitations"] = json!(["permission_denied"]);
    let snapshot = validate(&events).unwrap();
    assert!(!snapshot.complete);
    assert_eq!(snapshot.accounts.len(), 1);
    assert!(!snapshot.limitations.is_empty());
    events[4]["limitations"] = json!([]);
    rejected(&events, ProtocolError::Schema);
    events[4] = json!({"protocol":1,"id":"discover","event":"error","code":"permission_denied"});
    rejected(&events, ProtocolError::ProviderFailed);
    events[4]["message"] = json!("SENTINEL_SECRET");
    rejected(&events, ProtocolError::Schema);
}
#[test]
fn handshake_request_has_exact_fields_and_rejects_unsafe_instance_names() {
    let frame = handshake_request("test-main").unwrap();
    assert_eq!(frame.last(), Some(&b'\n'));
    assert_eq!(
        serde_json::from_slice::<Value>(&frame).unwrap(),
        json!({"protocol":1,"id":"handshake","method":"handshake","instance":"test-main"})
    );
    for instance in ["", "../evil", "a b", "0first", "a\nb", "a;touch SENTINEL"] {
        assert!(handshake_request(instance).is_err());
    }
}

#[test]
fn python_fixture_yields_group_derived_access_in_core() {
    let fixture = include_bytes!("../../../examples/external-provider/discovery.ndjson");
    let snapshot =
        validate_discovery(Cursor::new(fixture), "synthetic-example", "example-main").unwrap();
    let result =
        permesh_core::query_user(&[snapshot], &Default::default(), "alice@example.com").unwrap();
    assert_eq!(result.accounts.len(), 1);
    assert_eq!(result.access.len(), 1);
    let path = &result.access[0];
    assert_eq!(path.account.provider, "example-main");
    assert_eq!(path.groups[0].key.id, "backend");
    assert_eq!(path.resource.key.id, "repository");
    assert_eq!(path.grant.role, "read");
    assert_eq!(path.grant.certainty, permesh_core::Certainty::Observed);
    assert_eq!(
        path.memberships[0].provenance.observed_at,
        "2026-01-01T00:00:00Z"
    );
}

#[test]
fn record_flood_hits_budget_before_any_snapshot_can_escape() {
    let source = exchange();
    let mut input = bytes(&source[..1]);
    let record = bytes(&source[1..2]);
    for _ in 0..=permesh_provider_protocol::MAX_RECORDS {
        input.extend_from_slice(&record);
    }
    assert!(input.len() < permesh_provider_protocol::MAX_TRANSCRIPT_BYTES);
    let result = validate_discovery(Cursor::new(input), "test-provider", "test-main");
    assert_eq!(result.unwrap_err(), ProtocolError::RecordLimit);
}

#[test]
fn exactly_the_record_budget_can_form_a_valid_snapshot() {
    let mut header = exchange()[0].clone();
    header["capabilities"] = json!(["resources"]);
    let mut input = bytes(&[header]);
    for id in 0..permesh_provider_protocol::MAX_RECORDS {
        let record = json!({"protocol":1,"id":"discover","event":"record","kind":"resource","data":{"key":{"provider":"test-main","id":id.to_string()},"name":"Synthetic"}});
        serde_json::to_writer(&mut input, &record).unwrap();
        input.push(b'\n');
    }
    input.extend(bytes(&[json!({"protocol":1,"id":"discover","event":"complete","count":permesh_provider_protocol::MAX_RECORDS,"complete":true,"limitations":[]})]));
    let snapshot = validate_discovery(Cursor::new(input), "test-provider", "test-main").unwrap();
    assert!(snapshot.complete);
    assert_eq!(
        snapshot.resources.len(),
        permesh_provider_protocol::MAX_RECORDS
    );
}
