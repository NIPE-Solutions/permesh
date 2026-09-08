// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_protocol::{BrowserAuthDecoder, Progress};
use serde_json::json;
fn handshake() -> Vec<u8> {
    format!("{}\n",json!({"protocol":4,"id":"handshake","event":"handshake","provider":"fixture","capabilities":[],"draft":true})).into_bytes()
}
fn terminal() -> Vec<u8> {
    format!("{}\n",json!({"protocol":4,"id":"describe_auth","event":"auth","spec":{"schema_version":1,"authorization_endpoint":"https://id.example.test/authorize","token_endpoint":"https://id.example.test/token","scopes":["read"],"client_id_field":"client_id","refresh_token_slot":"refresh_token","authorization_parameters":{}}})).into_bytes()
}
#[test]
fn auth_declaration_requires_handshake_valid_spec_terminal_and_eof() {
    let mut decoder = BrowserAuthDecoder::new("fixture", "main", Some(&[])).unwrap();
    assert_eq!(
        decoder.push_frame(&handshake()).unwrap(),
        Progress::Handshake
    );
    assert_eq!(decoder.push_frame(&terminal()).unwrap(), Progress::Complete);
    assert_eq!(
        decoder.finish().unwrap().refresh_token_slot,
        "refresh_token"
    );
    let mut decoder = BrowserAuthDecoder::new("fixture", "main", None).unwrap();
    assert!(decoder.push_frame(&terminal()).is_err());
    assert!(decoder.finish().is_err());
    for invalid in [
        String::from_utf8(terminal()).unwrap().replace(
            "https://id.example.test/token",
            "http://id.example.test/token",
        ),
        String::from_utf8(terminal())
            .unwrap()
            .replace("\"protocol\":4", "\"protocol\":3"),
        String::from_utf8(terminal()).unwrap().replace(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
    ] {
        let mut decoder = BrowserAuthDecoder::new("fixture", "main", None).unwrap();
        decoder.push_frame(&handshake()).unwrap();
        assert!(decoder.push_frame(invalid.as_bytes()).is_err());
        assert!(decoder.finish().is_err());
    }
    let mut decoder = BrowserAuthDecoder::new("fixture", "main", None).unwrap();
    decoder.push_frame(&handshake()).unwrap();
    decoder.push_frame(&terminal()).unwrap();
    assert!(decoder.push_frame(&terminal()).is_err());
    assert!(decoder.finish().is_err());
}
