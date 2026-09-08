// SPDX-License-Identifier: MIT OR Apache-2.0
#![allow(clippy::unwrap_used)]
use permesh_provider_protocol::{
    DiscoveryDecoder, HealthDecoder, Progress, ProtocolError, handshake_request_versioned,
};
use permesh_provider_sdk::Capability;
use serde_json::{Value, json};
fn frame(value: Value) -> Vec<u8> {
    let mut result = serde_json::to_vec(&value).unwrap();
    result.push(b'\n');
    result
}
fn handshake(version: u32) -> Vec<u8> {
    frame(
        json!({"protocol":version,"id":"handshake","event":"handshake","provider":"fixture","capabilities":["accounts"],"draft":true}),
    )
}
fn health() -> Value {
    json!({"protocol":2,"id":"check","event":"health","status":"ok","limitations":[]})
}
fn decoder() -> HealthDecoder {
    HealthDecoder::new("fixture", "main", Some(&[Capability::Accounts])).unwrap()
}
#[test]
fn discovery_and_requests_pin_supported_versions() {
    for version in [1, 2] {
        let request: Value =
            serde_json::from_slice(&handshake_request_versioned("main", version).unwrap()).unwrap();
        assert_eq!(
            request,
            json!({"protocol":version,"id":"handshake","method":"handshake","instance":"main"})
        );
        let mut decoder = DiscoveryDecoder::new_versioned(
            "fixture",
            "main",
            Some(&[Capability::Accounts]),
            version,
        )
        .unwrap();
        assert_eq!(
            decoder.push_frame(&handshake(version)),
            Ok(Progress::Handshake)
        );
        assert_eq!(decoder.push_frame(&frame(json!({"protocol":version,"id":"discover","event":"complete","count":0,"complete":true,"limitations":[]}))),Ok(Progress::Complete));
        assert!(decoder.finish().unwrap().complete);
    }
    for version in [0, 3, u32::MAX] {
        assert!(matches!(
            DiscoveryDecoder::new_versioned("fixture", "main", None, version),
            Err(ProtocolError::Version)
        ));
        assert_eq!(
            handshake_request_versioned("main", version),
            Err(ProtocolError::Version)
        );
    }
    let mut decoder = DiscoveryDecoder::new_versioned("fixture", "main", None, 2).unwrap();
    assert_eq!(
        decoder.push_frame(&handshake(1)),
        Err(ProtocolError::Version)
    );
    assert_eq!(
        decoder.push_frame(&handshake(2)),
        Err(ProtocolError::Version)
    );
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Version);
}
#[test]
fn health_has_fixed_message_and_curated_limitations() {
    let mut decoder = decoder();
    assert_eq!(decoder.push_frame(&handshake(2)), Ok(Progress::Handshake));
    let mut terminal = health();
    terminal["limitations"] = json!(["permission_denied"]);
    assert_eq!(decoder.push_frame(&frame(terminal)), Ok(Progress::Complete));
    let health = decoder.finish().unwrap();
    assert_eq!(
        health.message,
        "Trusted external provider health check completed"
    );
    assert_eq!(
        health.limitations,
        vec!["External provider reports a permission denial"]
    );
}
#[test]
fn health_requires_handshake_exact_capabilities_completion_and_eof() {
    assert_eq!(decoder().finish().unwrap_err(), ProtocolError::Incomplete);
    let mut decoder = self::decoder();
    assert_eq!(
        decoder.push_frame(&frame(health())),
        Err(ProtocolError::Sequence)
    );
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Sequence);
    let mut decoder = HealthDecoder::new("fixture", "main", Some(&[])).unwrap();
    assert_eq!(
        decoder.push_frame(&handshake(2)),
        Err(ProtocolError::Capability)
    );
    let mut decoder = self::decoder();
    decoder.push_frame(&handshake(2)).unwrap();
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Incomplete);
    let mut decoder = self::decoder();
    decoder.push_frame(&handshake(2)).unwrap();
    decoder.push_frame(&frame(health())).unwrap();
    assert_eq!(
        decoder.push_frame(&frame(health())),
        Err(ProtocolError::Sequence)
    );
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Sequence);
}
#[test]
fn health_rejects_wrong_versions_ids_status_unknown_fields_and_limitations() {
    for (field, value, error) in [
        ("protocol", json!(1), ProtocolError::Version),
        ("id", json!("discover"), ProtocolError::Sequence),
        ("status", json!("failed"), ProtocolError::Schema),
        ("extra", json!("SENTINEL"), ProtocolError::Schema),
        ("limitations", json!(["SENTINEL"]), ProtocolError::Schema),
    ] {
        let mut decoder = decoder();
        decoder.push_frame(&handshake(2)).unwrap();
        let mut terminal = health();
        terminal[field] = value;
        assert_eq!(decoder.push_frame(&frame(terminal)), Err(error));
        assert_eq!(decoder.push_frame(&frame(health())), Err(error));
        assert_eq!(decoder.finish().unwrap_err(), error);
        assert!(!format!("{error}").contains("SENTINEL"));
    }
    let mut terminal = health();
    terminal.as_object_mut().unwrap().remove("limitations");
    let mut decoder = decoder();
    decoder.push_frame(&handshake(2)).unwrap();
    assert_eq!(
        decoder.push_frame(&frame(terminal)),
        Err(ProtocolError::Schema)
    );
}
#[test]
fn health_strict_frames_and_provider_errors_are_sticky() {
    for invalid in [
        b"{}".to_vec(),
        [handshake(2), frame(health())].concat(),
        b"{\"protocol\":2,\"protocol\":2}\n".to_vec(),
    ] {
        let mut decoder = decoder();
        let error = decoder.push_frame(&invalid).unwrap_err();
        assert_eq!(decoder.finish().unwrap_err(), error);
    }
    let mut decoder = decoder();
    decoder.push_frame(&handshake(2)).unwrap();
    assert_eq!(
        decoder.push_frame(&frame(
            json!({"protocol":2,"id":"check","event":"error","code":"authentication"})
        )),
        Err(ProtocolError::ProviderFailed)
    );
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::ProviderFailed);
}

#[test]
fn health_handshake_rejects_downgrade_wrong_provider_and_duplicate_caps() {
    for (key, value, error) in [
        ("protocol", json!(1), ProtocolError::Version),
        ("provider", json!("other"), ProtocolError::Provider),
        ("draft", json!(false), ProtocolError::Version),
        (
            "capabilities",
            json!(["accounts", "accounts"]),
            ProtocolError::Capability,
        ),
        ("extra", json!(true), ProtocolError::Schema),
    ] {
        let mut greeting: Value = serde_json::from_slice(&handshake(2)).unwrap();
        greeting[key] = value;
        let mut decoder = decoder();
        assert_eq!(decoder.push_frame(&frame(greeting)), Err(error));
        assert_eq!(decoder.push_frame(&handshake(2)), Err(error));
        assert_eq!(decoder.finish().unwrap_err(), error);
    }
}

#[test]
fn health_accepts_exact_frame_budget_and_rejects_overflow() {
    use permesh_provider_protocol::MAX_FRAME_BYTES;
    let mut decoder = decoder();
    let mut greeting = handshake(2);
    greeting.pop();
    greeting.resize(MAX_FRAME_BYTES - 1, b' ');
    greeting.push(b'\n');
    assert_eq!(decoder.push_frame(&greeting), Ok(Progress::Handshake));
    let mut terminal = frame(health());
    terminal.pop();
    terminal.resize(MAX_FRAME_BYTES - 1, b' ');
    terminal.push(b'\n');
    assert_eq!(decoder.push_frame(&terminal), Ok(Progress::Complete));
    assert!(decoder.finish().is_ok());
    greeting.push(b'\n');
    // The extra delimiter is a second frame, never accepted as whitespace.
    let mut decoder = self::decoder();
    assert!(decoder.push_frame(&greeting).is_err());
    let oversized = vec![b' '; MAX_FRAME_BYTES + 1];
    let mut decoder = self::decoder();
    assert_eq!(
        decoder.push_frame(&oversized),
        Err(ProtocolError::FrameLimit)
    );
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::FrameLimit);
}

#[test]
fn version_two_discovery_preserves_records_and_health_is_not_discovery() {
    let mut discovery =
        DiscoveryDecoder::new_versioned("fixture", "main", Some(&[Capability::Accounts]), 2)
            .unwrap();
    discovery.push_frame(&handshake(2)).unwrap();
    for id in ["z", "a"] {
        assert_eq!(discovery.push_frame(&frame(json!({"protocol":2,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"main","id":id},"login":id,"kind":"human","verified_emails":[]}}))), Ok(Progress::Record));
    }
    discovery.push_frame(&frame(json!({"protocol":2,"id":"discover","event":"complete","count":2,"complete":true,"limitations":[]}))).unwrap();
    let snapshot = discovery.finish().unwrap();
    assert_eq!(
        snapshot
            .accounts
            .iter()
            .map(|account| account.key.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "z"]
    );

    let mut decoder = decoder();
    decoder.push_frame(&handshake(2)).unwrap();
    assert_eq!(decoder.push_frame(&frame(json!({"protocol":2,"id":"check","event":"complete","count":0,"complete":true,"limitations":[]}))), Err(ProtocolError::Sequence));
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Sequence);
}
