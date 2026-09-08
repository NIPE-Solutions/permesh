// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_protocol::{
    DiscoveryDecoder, MAX_FRAME_BYTES, MAX_TRANSCRIPT_BYTES, Progress, ProtocolError,
};
use permesh_provider_sdk::Capability;
use serde_json::json;

fn frame(value: serde_json::Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}
fn handshake() -> Vec<u8> {
    frame(
        json!({"protocol":1,"id":"handshake","event":"handshake","provider":"test","capabilities":["accounts"],"draft":true}),
    )
}
fn complete() -> Vec<u8> {
    frame(
        json!({"protocol":1,"id":"discover","event":"complete","count":0,"complete":true,"limitations":[]}),
    )
}
fn decoder() -> DiscoveryDecoder {
    DiscoveryDecoder::new("test", "test-main", None).unwrap()
}

#[test]
fn progress_and_finish_require_completion() {
    let mut decoder = decoder();
    assert_eq!(decoder.push_frame(&handshake()), Ok(Progress::Handshake));
    assert_eq!(decoder.push_frame(&complete()), Ok(Progress::Complete));
    assert!(decoder.finish().unwrap().complete);
    assert_eq!(
        self::decoder().finish().unwrap_err(),
        ProtocolError::Incomplete
    );
}
#[test]
fn registration_must_match_before_handshake_progress() {
    for expected in [
        vec![],
        vec![Capability::Accounts, Capability::Resources],
        vec![Capability::Resources],
    ] {
        let mut decoder = DiscoveryDecoder::new("test", "test-main", Some(&expected)).unwrap();
        assert_eq!(
            decoder.push_frame(&handshake()),
            Err(ProtocolError::Capability)
        );
        assert_eq!(
            decoder.push_frame(&complete()),
            Err(ProtocolError::Capability)
        );
        assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Capability);
    }
    let mut decoder =
        DiscoveryDecoder::new("test", "test-main", Some(&[Capability::Accounts])).unwrap();
    assert_eq!(decoder.push_frame(&handshake()), Ok(Progress::Handshake));
}
#[test]
fn each_push_is_exactly_one_whole_frame_and_errors_poison() {
    for invalid in [
        vec![],
        b"{}".to_vec(),
        [handshake(), complete()].concat(),
        [handshake(), b"\n".to_vec()].concat(),
        b"{\"a\":1,\"a\":2}\n".to_vec(),
    ] {
        let mut decoder = decoder();
        let error = decoder.push_frame(&invalid).unwrap_err();
        assert_eq!(decoder.push_frame(&handshake()), Err(error));
        assert_eq!(decoder.finish().unwrap_err(), error);
    }
}
#[test]
fn late_frame_invalidates_completed_snapshot() {
    let mut decoder = decoder();
    decoder.push_frame(&handshake()).unwrap();
    decoder.push_frame(&complete()).unwrap();
    assert_eq!(
        decoder.push_frame(&complete()),
        Err(ProtocolError::Sequence)
    );
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Sequence);
}
#[test]
fn incremental_limits_count_lf_and_all_frames_once() {
    let mut oversized = vec![b' '; MAX_FRAME_BYTES];
    oversized.push(b'\n');
    assert_eq!(
        decoder().push_frame(&oversized),
        Err(ProtocolError::FrameLimit)
    );
    let mut decoder = decoder();
    let mut first = handshake();
    first.pop();
    first.resize(MAX_FRAME_BYTES - 1, b' ');
    first.push(b'\n');
    decoder.push_frame(&first).unwrap();
    // Each distinct account uses a complete one-MiB frame; no snapshot may escape
    // once the transcript budget is exceeded, even if a later completion is valid.
    for index in 1..=MAX_TRANSCRIPT_BYTES / MAX_FRAME_BYTES {
        let mut record = frame(
            json!({"protocol":1,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"test-main","id":index.to_string()},"login":"test","kind":"human","verified_emails":[]}}),
        );
        record.pop();
        record.resize(MAX_FRAME_BYTES - 1, b' ');
        record.push(b'\n');
        if index == MAX_TRANSCRIPT_BYTES / MAX_FRAME_BYTES {
            assert_eq!(
                decoder.push_frame(&record),
                Err(ProtocolError::TranscriptLimit)
            );
        } else {
            assert_eq!(decoder.push_frame(&record), Ok(Progress::Record));
        }
    }
    assert_eq!(
        decoder.finish().unwrap_err(),
        ProtocolError::TranscriptLimit
    );
}

#[test]
fn explicit_partial_completion_is_preserved_but_domain_errors_fail_finish() {
    let mut decoder = decoder();
    decoder.push_frame(&handshake()).unwrap();
    let record = frame(
        json!({"protocol":1,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"wrong-instance","id":"one"},"login":"test","kind":"human","verified_emails":[]}}),
    );
    assert_eq!(decoder.push_frame(&record), Ok(Progress::Record));
    let partial = frame(
        json!({"protocol":1,"id":"discover","event":"complete","count":1,"complete":false,"limitations":["permission_denied"]}),
    );
    assert_eq!(decoder.push_frame(&partial), Ok(Progress::Complete));
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Snapshot);

    let mut decoder = self::decoder();
    decoder.push_frame(&handshake()).unwrap();
    let partial = frame(
        json!({"protocol":1,"id":"discover","event":"complete","count":0,"complete":false,"limitations":["permission_denied"]}),
    );
    decoder.push_frame(&partial).unwrap();
    let snapshot = decoder.finish().unwrap();
    assert!(!snapshot.complete);
    assert_eq!(snapshot.limitations.len(), 1);
}

#[test]
fn exact_transcript_budget_can_finish_successfully() {
    let mut decoder = decoder();
    let mut padded = handshake();
    padded.pop();
    padded.resize(MAX_FRAME_BYTES - 1, b' ');
    padded.push(b'\n');
    decoder.push_frame(&padded).unwrap();
    let records = MAX_TRANSCRIPT_BYTES / MAX_FRAME_BYTES - 2;
    for index in 0..records {
        let mut record = frame(
            json!({"protocol":1,"id":"discover","event":"record","kind":"account","data":{"key":{"provider":"test-main","id":index.to_string()},"login":"test","kind":"human","verified_emails":[]}}),
        );
        record.pop();
        record.resize(MAX_FRAME_BYTES - 1, b' ');
        record.push(b'\n');
        decoder.push_frame(&record).unwrap();
    }
    let mut completion = frame(
        json!({"protocol":1,"id":"discover","event":"complete","count":records,"complete":true,"limitations":[]}),
    );
    completion.pop();
    completion.resize(MAX_FRAME_BYTES - 1, b' ');
    completion.push(b'\n');
    decoder.push_frame(&completion).unwrap();
    assert_eq!(decoder.finish().unwrap().accounts.len(), records);
}

#[test]
fn capability_order_does_not_matter_and_unknown_fields_poison() {
    let mut decoder = DiscoveryDecoder::new(
        "test",
        "test-main",
        Some(&[Capability::Resources, Capability::Accounts]),
    )
    .unwrap();
    let greeting = frame(
        json!({"protocol":1,"id":"handshake","event":"handshake","provider":"test","capabilities":["accounts","resources"],"draft":true}),
    );
    assert_eq!(decoder.push_frame(&greeting), Ok(Progress::Handshake));
    let invalid = frame(
        json!({"protocol":1,"id":"discover","event":"complete","count":0,"complete":true,"limitations":[],"extra":"secret"}),
    );
    assert_eq!(decoder.push_frame(&invalid), Err(ProtocolError::Schema));
    assert_eq!(decoder.push_frame(&complete()), Err(ProtocolError::Schema));
    assert_eq!(decoder.finish().unwrap_err(), ProtocolError::Schema);
}
