// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_protocol::{
    Progress, ProtocolError,
    negotiated::{self, DiscoveryDecoder, HealthDecoder, Operation},
};
use permesh_provider_sdk::Capability;
use serde_json::{Value, json};
const CAPS: &[Capability] = &[
    Capability::Accounts,
    Capability::Identities,
    Capability::Resources,
    Capability::Groups,
    Capability::Memberships,
    Capability::Grants,
];
fn frame(v: Value) -> Vec<u8> {
    let mut b = serde_json::to_vec(&v).unwrap();
    b.push(b'\n');
    b
}
fn handshake() -> Value {
    json!({"protocol_version":1,"id":"handshake","event":"handshake","provider":"example","capabilities":["accounts","identities","resources","groups","memberships","grants"],"operations":["discover","check"],"draft":true})
}
fn decoder() -> DiscoveryDecoder {
    DiscoveryDecoder::new("example", "work", Some(CAPS)).unwrap()
}
fn complete(count: usize) -> Vec<u8> {
    frame(
        json!({"protocol_version":1,"id":"discover","event":"complete","count":count,"complete":true,"limitations":[]}),
    )
}
#[test]
fn request_pins_wire_and_operation() {
    assert_eq!(
        serde_json::from_slice::<Value>(
            &negotiated::handshake_request("work", Operation::Discover).unwrap()
        )
        .unwrap(),
        json!({"protocol_version":1,"id":"handshake","method":"handshake","instance":"work","operation":"discover"})
    );
    assert!(negotiated::handshake_request("bad/name", Operation::Check).is_err());
}
#[test]
fn literal_rich_fixture_maps_every_dimension() {
    let mut d = decoder();
    for line in include_str!("fixtures/negotiated-v1.ndjson").split_inclusive('\n') {
        d.push_frame(line.as_bytes()).unwrap();
    }
    let snapshot = d.finish().unwrap();
    assert_eq!(
        snapshot.accounts[0].status,
        permesh_core::IdentityStatus::Suspended
    );
    assert_eq!(
        snapshot.accounts[0].affiliation,
        permesh_core::Affiliation::External
    );
    assert_eq!(
        snapshot.identities[0].kind,
        permesh_core::IdentityKind::Service
    );
    assert_eq!(
        snapshot.identities[0].status,
        permesh_core::IdentityStatus::Active
    );
    assert_eq!(snapshot.resources[1].parent.as_ref().unwrap().id, "root");
    assert_eq!(
        snapshot.grants[0].certainty,
        permesh_core::Certainty::Derived
    );
    assert_eq!(
        snapshot.grants[0].evidence_kind,
        permesh_core::EvidenceKind::PolicyAttachment
    );
}
#[test]
fn handshake_requires_operation_and_exact_known_capabilities() {
    for (field, value) in [
        ("operations", json!(["check"])),
        ("operations", json!(["discover", "discover"])),
        ("operations", json!(["discover", "bad/name"])),
        ("operations", json!(["discover", ""])),
        (
            "operations",
            json!(
                std::iter::once("discover".to_owned())
                    .chain((0..16).map(|i| format!("op{i}")))
                    .collect::<Vec<_>>()
            ),
        ),
        ("capabilities", json!(["accounts"])),
        ("capabilities", json!(["other"])),
        ("draft", json!(false)),
        ("protocol_version", json!(2)),
        ("provider", json!("other")),
    ] {
        let mut d = decoder();
        let mut h = handshake();
        h[field] = value;
        assert!(d.push_frame(&frame(h)).is_err(), "{field}");
        assert!(d.finish().is_err());
    }
    let mut d = decoder();
    let mut h = handshake();
    h["operations"] = json!(["discover", "future_feature"]);
    assert_eq!(d.push_frame(&frame(h)).unwrap(), Progress::Handshake);
}
#[test]
fn strict_framing_fields_and_order_poison_the_decoder() {
    for bytes in [
        b"{\"protocol_version\":1,\"protocol_version\":1}\n".to_vec(),
        b"{}".to_vec(),
        b"{}\n{}\n".to_vec(),
        frame({
            let mut h = handshake();
            h["extra"] = json!(1);
            h
        }),
    ] {
        let mut d = decoder();
        assert!(d.push_frame(&bytes).is_err());
        assert!(d.push_frame(&frame(handshake())).is_err());
        assert!(d.finish().is_err());
    }
    let mut d = decoder();
    d.push_frame(&frame(handshake())).unwrap();
    d.push_frame(&complete(0)).unwrap();
    assert_eq!(d.push_frame(&complete(0)), Err(ProtocolError::Sequence));
    assert!(d.finish().is_err());
}
#[test]
fn health_is_negotiated_and_redacted() {
    let mut d = HealthDecoder::new("example", "work", Some(CAPS)).unwrap();
    let mut h = handshake();
    h["operations"] = json!(["discover"]);
    assert!(d.push_frame(&frame(h)).is_err());
    let mut d = HealthDecoder::new("example", "work", Some(CAPS)).unwrap();
    d.push_frame(&frame(handshake())).unwrap();
    assert_eq!(d.push_frame(&frame(json!({"protocol_version":1,"id":"check","event":"health","status":"ok","limitations":["rate_limited"]}))).unwrap(),Progress::Complete);
    assert_eq!(d.finish().unwrap().limitations.len(), 1);
    for code in [
        json!("api.rate_limit"),
        json!("SECRET VALUE"),
        json!("x".repeat(65)),
        json!(null),
    ] {
        let mut d = decoder();
        d.push_frame(&frame(handshake())).unwrap();
        let err=d.push_frame(&frame(json!({"protocol_version":1,"id":"discover","event":"error","code":"internal","provider_code":code}))).unwrap_err();
        assert!(!err.to_string().contains("api.rate_limit"));
        assert!(!err.to_string().contains("SECRET"));
        assert!(d.finish().is_err());
    }
}
#[test]
fn missing_dimensions_and_unknown_nested_fields_reject() {
    let fixture: Vec<Value> = include_str!("fixtures/negotiated-v1.ndjson")
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    for (index, field) in [
        (1, "affiliation"),
        (1, "status"),
        (2, "kind"),
        (2, "status"),
        (3, "parent"),
        (3, "kind"),
        (7, "evidence_kind"),
    ] {
        let mut d = decoder();
        d.push_frame(&frame(handshake())).unwrap();
        let mut record = fixture[index].clone();
        record["data"].as_object_mut().unwrap().remove(field);
        assert!(d.push_frame(&frame(record)).is_err(), "{index}:{field}");
    }
    let mut d = decoder();
    d.push_frame(&frame(handshake())).unwrap();
    let mut r = fixture[2].clone();
    r["data"]["key"]["extra"] = json!(true);
    assert!(d.push_frame(&frame(r)).is_err());
}
#[test]
fn whole_graph_references_and_cycles_validate_only_at_finish() {
    for parent in ["missing", "root"] {
        let mut d = decoder();
        d.push_frame(&frame(handshake())).unwrap();
        d.push_frame(&frame(json!({"protocol_version":1,"id":"discover","event":"record","kind":"resource","data":{"key":{"provider":"work","id":"root"},"name":"Root","kind":"example.folder","parent":{"provider":"work","id":parent}}}))).unwrap();
        d.push_frame(&complete(1)).unwrap();
        assert_eq!(d.finish().unwrap_err(), ProtocolError::Snapshot);
    }
    let mut d = decoder();
    d.push_frame(&frame(handshake())).unwrap();
    assert_eq!(d.push_frame(&complete(1)), Err(ProtocolError::Count));
    assert_eq!(decoder().finish().unwrap_err(), ProtocolError::Incomplete);
}

#[test]
fn literal_record_dtos_round_trip_without_domain_serialization() {
    for value in include_str!("fixtures/negotiated-v1.ndjson")
        .lines()
        .skip(1)
        .take(7)
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
    {
        let data = json!({"kind":value["kind"],"data":value["data"]});
        let record: negotiated::records::Record = serde_json::from_value(data.clone()).unwrap();
        assert_eq!(serde_json::to_value(record).unwrap(), data);
    }
}

#[test]
fn provider_code_is_optional_restricted_and_never_reflected() {
    for code in [
        None,
        Some(json!("a")),
        Some(json!("auth.rate_limit-1_2")),
        Some(json!("x".repeat(64))),
    ] {
        let mut d = decoder();
        d.push_frame(&frame(handshake())).unwrap();
        let mut event =
            json!({"protocol_version":1,"id":"discover","event":"error","code":"internal"});
        if let Some(code) = code {
            event["provider_code"] = code;
        }
        assert_eq!(
            d.push_frame(&frame(event)),
            Err(ProtocolError::ProviderFailed)
        );
        assert_eq!(d.finish().unwrap_err(), ProtocolError::ProviderFailed);
    }
    for code in [
        json!(""),
        json!("x".repeat(65)),
        json!("Upper"),
        json!("space secret"),
        json!("a/b"),
        json!("é"),
        json!(null),
        json!(1),
    ] {
        let mut d = decoder();
        d.push_frame(&frame(handshake())).unwrap();
        assert_eq!(d.push_frame(&frame(json!({"protocol_version":1,"id":"discover","event":"error","code":"internal","provider_code":code}))),Err(ProtocolError::Schema));
    }
}

#[test]
fn optional_operations_never_authorize_records_or_health() {
    let mut d = DiscoveryDecoder::new("example", "work", Some(&[])).unwrap();
    let mut h = handshake();
    h["capabilities"] = json!([]);
    h["operations"] = json!(["discover", "resources"]);
    d.push_frame(&frame(h)).unwrap();
    assert_eq!(d.push_frame(&frame(json!({"protocol_version":1,"id":"discover","event":"record","kind":"resource","data":{"key":{"provider":"work","id":"root"},"name":"Root","kind":null,"parent":null}}))),Err(ProtocolError::Capability));
    for event in [
        json!({"protocol_version":1,"id":"discover","event":"health","status":"ok","limitations":[]}),
        json!({"protocol_version":1,"id":"check","event":"complete","count":0,"complete":true,"limitations":[]}),
    ] {
        let mut d = decoder();
        d.push_frame(&frame(handshake())).unwrap();
        assert_eq!(d.push_frame(&frame(event)), Err(ProtocolError::Sequence));
    }
}

#[test]
fn partial_requires_curated_limitations_and_error_discards_prior_records() {
    let mut d = decoder();
    d.push_frame(&frame(handshake())).unwrap();
    d.push_frame(&frame(json!({"protocol_version":1,"id":"discover","event":"complete","count":0,"complete":false,"limitations":["visibility_limited"]}))).unwrap();
    let snapshot = d.finish().unwrap();
    assert!(!snapshot.complete);
    assert_eq!(snapshot.limitations.len(), 1);
    let mut d = decoder();
    d.push_frame(&frame(handshake())).unwrap();
    assert_eq!(d.push_frame(&frame(json!({"protocol_version":1,"id":"discover","event":"complete","count":0,"complete":false,"limitations":[]}))),Err(ProtocolError::Schema));
    let mut d = decoder();
    for line in include_str!("fixtures/negotiated-v1.ndjson")
        .split_inclusive('\n')
        .take(3)
    {
        d.push_frame(line.as_bytes()).unwrap();
    }
    assert_eq!(
        d.push_frame(&frame(
            json!({"protocol_version":1,"id":"discover","event":"error","code":"authentication"})
        )),
        Err(ProtocolError::ProviderFailed)
    );
    assert!(d.finish().is_err());
}

#[test]
fn new_enums_reject_legacy_ambiguity_and_unknown_values() {
    for (index, field, value) in [
        (1, "kind", "external"),
        (1, "status", "service"),
        (2, "affiliation", "partner"),
        (7, "certainty", "calculated"),
        (7, "evidence_kind", "direct"),
    ] {
        let mut record: Value = serde_json::from_str(
            include_str!("fixtures/negotiated-v1.ndjson")
                .lines()
                .nth(index)
                .unwrap(),
        )
        .unwrap();
        record["data"][field] = json!(value);
        let mut d = decoder();
        d.push_frame(&frame(handshake())).unwrap();
        assert_eq!(d.push_frame(&frame(record)), Err(ProtocolError::Schema));
    }
}

#[test]
fn operation_names_have_bounded_ascii_syntax_and_frame_limits_apply() {
    for operation in [
        "a".repeat(65),
        "0operation".into(),
        "é".into(),
        "a.b".into(),
    ] {
        let mut h = handshake();
        h["operations"] = json!(["discover", operation]);
        assert_eq!(
            decoder().push_frame(&frame(h)),
            Err(ProtocolError::Capability)
        );
    }
    let mut h = handshake();
    let mut ops = (0..15)
        .map(|i| format!("Future_{i}-op"))
        .collect::<Vec<_>>();
    ops.push("discover".into());
    h["operations"] = json!(ops);
    assert_eq!(decoder().push_frame(&frame(h)), Ok(Progress::Handshake));
    let mut d = decoder();
    assert_eq!(
        d.push_frame(&vec![b' '; permesh_provider_protocol::MAX_FRAME_BYTES + 1]),
        Err(ProtocolError::FrameLimit)
    );
    assert_eq!(d.finish().unwrap_err(), ProtocolError::FrameLimit);
}

#[test]
fn negotiated_version_one_is_distinct_from_legacy_protocol_one() {
    assert_eq!(negotiated::PROTOCOL_VERSION, 1);
    for event in [
        json!({"protocol":1,"id":"handshake","event":"handshake","provider":"example","capabilities":["accounts","identities","resources","groups","memberships","grants"],"operations":["discover","check"],"draft":true}),
        json!({"protocol":1,"protocol_version":1,"id":"handshake","event":"handshake","provider":"example","capabilities":["accounts","identities","resources","groups","memberships","grants"],"operations":["discover","check"],"draft":true}),
    ] {
        let mut d = decoder();
        assert_eq!(d.push_frame(&frame(event)), Err(ProtocolError::Schema));
        assert_eq!(d.finish().unwrap_err(), ProtocolError::Schema);
    }
    let mut d = decoder();
    assert_eq!(d.push_frame(&frame(handshake())), Ok(Progress::Handshake));
    assert_eq!(d.push_frame(&frame(json!({"protocol":1,"id":"discover","event":"complete","count":0,"complete":true,"limitations":[]}))), Err(ProtocolError::Schema));
    let mut legacy =
        permesh_provider_protocol::DiscoveryDecoder::new("example", "work", Some(CAPS)).unwrap();
    assert_eq!(
        legacy.push_frame(&frame(handshake())),
        Err(ProtocolError::Schema)
    );
}

#[test]
fn network_feature_request_is_explicit_and_preserves_absent_feature_bytes() {
    use negotiated::Feature::NetworkV1;
    for operation in [Operation::Discover, Operation::Check] {
        assert_eq!(
            negotiated::handshake_request("work", operation).unwrap(),
            negotiated::handshake_request_with_features("work", operation, &[]).unwrap()
        );
        let request: Value = serde_json::from_slice(
            &negotiated::handshake_request_with_features("work", operation, &[NetworkV1]).unwrap(),
        )
        .unwrap();
        assert_eq!(request["features"], json!(["network_v1"]));
        assert!(
            negotiated::handshake_request_with_features("work", operation, &[NetworkV1, NetworkV1])
                .is_err()
        );
    }
}

#[test]
fn required_network_feature_is_exact_and_failure_is_irreversible() {
    use negotiated::Feature::NetworkV1;
    for features in [
        None,
        Some(Value::Null),
        Some(json!([])),
        Some(json!(["unknown"])),
        Some(json!(["network_v1", "network_v1"])),
        Some(json!(["network_v1", "unknown"])),
        Some(json!([{"network_v1":null}])),
        Some(json!("network_v1")),
    ] {
        let mut response = handshake();
        if let Some(features) = features {
            response["features"] = features;
        }
        let mut discovery =
            DiscoveryDecoder::with_required_features("example", "work", Some(CAPS), &[NetworkV1])
                .unwrap();
        let mut health =
            HealthDecoder::with_required_features("example", "work", Some(CAPS), &[NetworkV1])
                .unwrap();
        assert!(discovery.push_frame(&frame(response.clone())).is_err());
        assert!(health.push_frame(&frame(response)).is_err());
        let mut corrected = handshake();
        corrected["features"] = json!(["network_v1"]);
        assert!(discovery.push_frame(&frame(corrected.clone())).is_err());
        assert!(health.push_frame(&frame(corrected)).is_err());
        assert!(discovery.finish().is_err());
        assert!(health.finish().is_err());
    }
}

#[test]
fn network_feature_success_is_opt_in_for_discovery_and_health() {
    use negotiated::Feature::NetworkV1;
    let mut response = handshake();
    response["features"] = json!(["network_v1"]);
    assert!(decoder().push_frame(&frame(response.clone())).is_err());
    assert!(
        HealthDecoder::new("example", "work", Some(CAPS))
            .unwrap()
            .push_frame(&frame(response.clone()))
            .is_err()
    );
    let mut discovery =
        DiscoveryDecoder::with_required_features("example", "work", Some(CAPS), &[NetworkV1])
            .unwrap();
    let mut health =
        HealthDecoder::with_required_features("example", "work", Some(CAPS), &[NetworkV1]).unwrap();
    assert_eq!(
        discovery.push_frame(&frame(response.clone())),
        Ok(Progress::Handshake)
    );
    assert_eq!(health.push_frame(&frame(response)), Ok(Progress::Handshake));
    discovery.push_frame(&complete(0)).unwrap();
    health
        .push_frame(&frame(
            json!({"protocol_version":1,"id":"check","event":"health","status":"ok","limitations":[]}),
        ))
        .unwrap();
    assert!(discovery.finish().unwrap().complete);
    assert!(health.finish().is_ok());
    let mut empty = handshake();
    empty["features"] = json!([]);
    assert!(decoder().push_frame(&frame(empty)).is_err());
}
