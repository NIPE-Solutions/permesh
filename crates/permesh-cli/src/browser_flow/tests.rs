// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use super::*;
use serde_json::json;
fn spec() -> BrowserAuthSpec {
    serde_json::from_value(json!({"schema_version":1,"authorization_endpoint":"https://id.example.test/authorize","token_endpoint":"https://id.example.test/token","scopes":["read"],"client_id_field":"client_id","refresh_token_slot":"refresh_token","authorization_parameters":{}})).unwrap()
}
#[tokio::test]
async fn authorization_uses_random_state_s256_and_loopback_and_cancels() {
    let pending = Pending::begin(spec(), "client".into()).await.unwrap();
    let pairs: std::collections::BTreeMap<_, _> =
        pending.url().query_pairs().into_owned().collect();
    assert_eq!(pairs["response_type"], "code");
    assert_eq!(pairs["code_challenge_method"], "S256");
    assert_eq!(pairs["scope"], "read");
    assert_eq!(pairs["state"].len(), 43);
    assert_eq!(
        pairs["code_challenge"],
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(pending.verifier.expose()))
    );
    let redirect = url::Url::parse(&pairs["redirect_uri"]).unwrap();
    assert_eq!(redirect.host_str(), Some("127.0.0.1"));
    assert!(redirect.port().unwrap() > 0);
    assert_eq!(redirect.path(), "/oauth/callback");
    let other = Pending::begin(spec(), "client".into()).await.unwrap();
    assert_ne!(pending.state.expose(), other.state.expose());
    assert_ne!(pending.verifier.expose(), other.verifier.expose());
    let cancel = Cancellation::new();
    cancel.cancel();
    assert!(matches!(
        pending.finish(None, &cancel).await,
        Err(FlowError::Cancelled)
    ));
}

#[tokio::test]
async fn client_secret_reflections_are_rejected_before_url_display() {
    let secret = Secret::new("CLIENT/SECRET+value".into());
    let pending = Pending::begin(spec(), secret.expose().into())
        .await
        .unwrap();
    assert_eq!(
        pending.validate_secret(Some(&secret)),
        Err(FlowError::Configuration)
    );
}
#[tokio::test]
async fn real_loopback_rejects_wrong_state_then_exchanges_bound_code() {
    let (endpoint,request)=token::tests::mock(200,"",r#"{"access_token":"EPHEMERAL_ACCESS","refresh_token":"STORED_REFRESH","token_type":"Bearer","scope":"read"}"#).await;
    let mut pending = Pending::begin(spec(), "client".into()).await.unwrap();
    pending.token_endpoint = endpoint;
    let port = pending.listener.local_addr().unwrap().port();
    let state = pending.state.expose().to_string();
    let verifier = pending.verifier.expose().to_string();
    let cancel = Cancellation::new();
    let task = tokio::spawn(async move { pending.finish(None, &cancel).await });
    for (state, expected) in [("wrong", "400"), (state.as_str(), "200")] {
        let mut stream = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        stream.write_all(format!("GET /oauth/callback?state={state}&code=CODE_SENTINEL HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with(&format!("HTTP/1.1 {expected}")));
        assert!(!response.contains("CODE_SENTINEL"));
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        assert!(headers.contains(&format!("Content-Length: {}", body.len())));
    }
    assert_eq!(task.await.unwrap().unwrap().expose(), "STORED_REFRESH");
    let request = request.await.unwrap();
    assert!(request.contains("code=CODE_SENTINEL"));
    assert!(request.contains(&format!("code_verifier={verifier}")));
    assert!(request.contains("grant_type=authorization_code"));
}
#[tokio::test]
async fn cancellation_and_deadline_close_listener_without_a_token() {
    let pending = Pending::begin(spec(), "client".into()).await.unwrap();
    let port = pending.listener.local_addr().unwrap().port();
    let cancel = Cancellation::new();
    let signal = cancel.clone();
    let task = tokio::spawn(async move { pending.finish(None, &cancel).await });
    let _stream = tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .unwrap();
    signal.cancel();
    assert!(matches!(task.await.unwrap(), Err(FlowError::Cancelled)));
    assert!(
        tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .is_err()
    );
    let mut pending = Pending::begin(spec(), "client".into()).await.unwrap();
    pending.deadline = tokio::time::Instant::now() + Duration::from_millis(10);
    assert!(matches!(
        pending.finish(None, &Cancellation::new()).await,
        Err(FlowError::Timeout)
    ));
}
