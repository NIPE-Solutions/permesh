// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_protocol::{DiscoveryDecoder, ProtocolError, handshake_request_versioned};
use serde_json::json;
#[test]
fn describe_handshake_is_pinned_to_three_without_answers() {
    let request = handshake_request_versioned("fixture-main", 3).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request).unwrap(),
        json!({"protocol":3,"id":"handshake","method":"handshake","instance":"fixture-main"})
    );
    assert_eq!(request.last(), Some(&b'\n'));
}
#[test]
fn draft_three_never_enables_discovery() {
    assert!(matches!(
        DiscoveryDecoder::new_versioned("fixture", "fixture-main", None, 3),
        Err(ProtocolError::Version)
    ));
    assert!(matches!(
        handshake_request_versioned("fixture-main", 5),
        Err(ProtocolError::Version)
    ));
}

fn frame(value: serde_json::Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}
fn handshake(version: u32) -> Vec<u8> {
    frame(
        json!({"protocol":version,"id":"handshake","event":"handshake","provider":"fixture","capabilities":[],"draft":true}),
    )
}
fn spec() -> serde_json::Value {
    json!({"schema_version":1,"title":"Fixture setup","description":"Configure the fixture","steps":[{"id":"connection","title":"Connection","description":"Connection options","fields":[{"key":"tenant","label":"Tenant","help":"Directory name","required":true,"default":"acme","input":{"type":"text","min_length":1,"max_length":128}}]}]})
}
fn terminal() -> Vec<u8> {
    frame(json!({"protocol":3,"id":"describe","event":"setup","spec":spec()}))
}
#[test]
fn validated_setup_is_terminal_and_sticky_until_eof() {
    use permesh_provider_protocol::{Progress, SetupDecoder};
    let mut decoder = SetupDecoder::new("fixture", "fixture-main", Some(&[])).unwrap();
    assert_eq!(decoder.push_frame(&handshake(3)), Ok(Progress::Handshake));
    assert_eq!(decoder.push_frame(&terminal()), Ok(Progress::Complete));
    let spec = decoder.finish().unwrap();
    spec.validate().unwrap();
    assert_eq!(spec.title, "Fixture setup");
    let mut decoder = SetupDecoder::new("fixture", "fixture-main", Some(&[])).unwrap();
    decoder.push_frame(&handshake(3)).unwrap();
    decoder.push_frame(&terminal()).unwrap();
    assert_eq!(
        decoder.push_frame(&terminal()),
        Err(ProtocolError::Sequence)
    );
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Sequence);
}

#[test]
fn setup_rejects_untrusted_schema_and_poison_cannot_recover() {
    use permesh_provider_protocol::SetupDecoder;
    let mut bad_specs = Vec::new();
    let mut bad = spec();
    bad["extra"] = json!("SENTINEL");
    bad_specs.push(bad);
    let mut bad = spec();
    bad["steps"][0]["fields"][0]["input"]["extra"] = json!(true);
    bad_specs.push(bad);
    let mut bad = spec();
    bad["steps"][0]["fields"][0]["input"] = json!({"type":"credential"});
    bad_specs.push(bad);
    let mut bad = spec();
    bad["title"] = json!("SENTINEL\u{001b}");
    bad_specs.push(bad);
    let mut bad = spec();
    bad["steps"][0]["when"] = json!({"field":"tenant","equals":"acme"});
    bad_specs.push(bad);
    let mut bad = spec();
    bad["description"] = json!("x".repeat(65536));
    bad_specs.push(bad);
    for bad in bad_specs {
        let mut decoder = SetupDecoder::new("fixture", "main", Some(&[])).unwrap();
        decoder.push_frame(&handshake(3)).unwrap();
        let error = decoder
            .push_frame(&frame(
                json!({"protocol":3,"id":"describe","event":"setup","spec":bad}),
            ))
            .unwrap_err();
        assert_eq!(error, ProtocolError::Schema);
        assert!(!error.to_string().contains("SENTINEL"));
        assert_eq!(decoder.push_frame(&terminal()), Err(error));
        assert_eq!(decoder.finish().unwrap_err(), error);
    }
}
#[test]
fn setup_requires_matching_handshake_operation_and_terminal() {
    use permesh_provider_protocol::SetupDecoder;
    for version in [1, 2, 4] {
        let mut decoder = SetupDecoder::new("fixture", "main", Some(&[])).unwrap();
        assert_eq!(
            decoder.push_frame(&handshake(version)),
            Err(ProtocolError::Version)
        );
    }
    for change in [
        json!({"provider":"wrong"}),
        json!({"capabilities":["accounts"]}),
        json!({"id":"describe"}),
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&handshake(3)).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        let mut decoder = SetupDecoder::new("fixture", "main", Some(&[])).unwrap();
        assert!(decoder.push_frame(&frame(value)).is_err());
    }
    for change in [
        json!({"id":"discover"}),
        json!({"protocol":2}),
        json!({"extra":true}),
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&terminal()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        let mut decoder = SetupDecoder::new("fixture", "main", None).unwrap();
        decoder.push_frame(&handshake(3)).unwrap();
        assert!(decoder.push_frame(&frame(value)).is_err());
    }
    let mut decoder = SetupDecoder::new("fixture", "main", None).unwrap();
    decoder.push_frame(&handshake(3)).unwrap();
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Incomplete);
    let mut decoder = SetupDecoder::new("fixture", "main", None).unwrap();
    assert!(decoder.push_frame(&terminal()).is_err());
}
#[test]
fn setup_uses_shared_duplicate_key_rejection() {
    use permesh_provider_protocol::SetupDecoder;
    let mut decoder = SetupDecoder::new("fixture", "main", None).unwrap();
    decoder.push_frame(&handshake(3)).unwrap();
    let bytes = String::from_utf8(terminal()).unwrap().replace(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
    );
    assert!(decoder.push_frame(bytes.as_bytes()).is_err());
}

#[test]
fn setup_enforces_total_spec_budget_inside_larger_transport_budget() {
    use permesh_provider_protocol::{Progress, SetupDecoder};
    let mut value = spec();
    let fields:Vec<_>=(0..70).map(|index|json!({"key":format!("field_{index}"),"label":"Field","help":"x".repeat(800),"required":false,"input":{"type":"boolean"}})).collect();
    value["steps"][0]["fields"] = json!(fields);
    let accepted: permesh_provider_sdk::setup::SetupSpec =
        serde_json::from_value(value.clone()).unwrap();
    accepted.validate().unwrap();
    for field in value["steps"][0]["fields"].as_array_mut().unwrap() {
        field["help"] = json!("x".repeat(1024));
    }
    assert!(serde_json::to_vec(&value).unwrap().len() > 65536);
    let bytes = frame(json!({"protocol":3,"id":"describe","event":"setup","spec":value}));
    assert!(bytes.len() < permesh_provider_protocol::MAX_FRAME_BYTES);
    let mut decoder = SetupDecoder::new("fixture", "main", None).unwrap();
    assert_eq!(decoder.push_frame(&handshake(3)), Ok(Progress::Handshake));
    assert_eq!(decoder.push_frame(&bytes), Err(ProtocolError::Schema));
}
