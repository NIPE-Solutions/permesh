// SPDX-License-Identifier: MIT
//! Loopback HTTP is a test-only transport seam; production construction requires HTTPS.
#![allow(clippy::unwrap_used)]
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn config() -> CredentialResolver {
    serde_json::from_value(serde_json::json!({"type":"vault_kv2","version":1,"origin":"https://vault.example","mount":"secret","path":"team/token","field":"token","bootstrap":"env://PERMESH_TEST_ABSENT_REMOTE_BOOTSTRAP_517103"})).unwrap()
}
async fn server(response: String) -> (Url, tokio::task::JoinHandle<String>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        loop {
            let mut byte = [0];
            if stream.read_exact(&mut byte).await.is_err() {
                break;
            }
            request.push(byte[0]);
            if request.ends_with(b"\r\n\r\n") {
                break;
            }
            assert!(request.len() < 8192);
        }
        let _ = stream.write_all(response.as_bytes()).await;
        String::from_utf8(request).unwrap()
    });
    (
        Url::parse(&format!("http://{address}/v1/secret/data/team/token")).unwrap(),
        task,
    )
}
fn seam(url: Url) -> Prepared {
    Prepared {
        config: config(),
        client: Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
        url,
    }
}
fn response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
#[tokio::test]
async fn exact_get_authentication_and_field_delivery() {
    let (url,request)=server(response("200 OK",r#"{"data":{"data":{"token":"selected-sentinel","unrelated":"never-delivered"},"metadata":{"version":4,"deletion_time":"","destroyed":false}}}"#)).await;
    let result = seam(url)
        .request(Secret::new("bootstrap-sentinel".into()))
        .await
        .unwrap();
    assert_eq!(result.secret.expose(), "selected-sentinel");
    assert_eq!(result.version, 4);
    let request = request.await.unwrap();
    assert!(request.starts_with("GET /v1/secret/data/team/token HTTP/1.1\r\n"));
    assert!(request.contains("x-vault-token: bootstrap-sentinel\r\n"));
    assert!(!request.contains("authorization:"));
}
#[tokio::test]
async fn redirects_never_reach_other_origin_and_bodies_never_reflect() {
    let destination = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (url,request)=server(format!("HTTP/1.1 302 Found\r\nLocation: http://{}/stolen\r\nContent-Length: 15\r\nConnection: close\r\n\r\nsecret-sentinel",destination.local_addr().unwrap())).await;
    let error = seam(url)
        .request(Secret::new("bootstrap-sentinel".into()))
        .await
        .err()
        .unwrap();
    assert!(error.message.contains("redirects"));
    assert!(!error.message.contains("sentinel"));
    request.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), destination.accept())
            .await
            .is_err()
    );
    for status in [
        "401 Unauthorized",
        "403 Forbidden",
        "404 Not Found",
        "500 Failed",
    ] {
        let (url, request) = server(response(status, "secret-sentinel")).await;
        let error = seam(url)
            .request(Secret::new("bootstrap-sentinel".into()))
            .await
            .err()
            .unwrap();
        assert_eq!(error.code, 3);
        assert!(!error.message.contains("sentinel"));
        request.await.unwrap();
    }
}
#[tokio::test]
async fn declared_and_streamed_oversized_responses_fail_closed() {
    for raw in [
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MAX_RESPONSE + 1
        ),
        format!(
            "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{}",
            "x".repeat(MAX_RESPONSE + 1)
        ),
    ] {
        let (url, request) = server(raw).await;
        let error = seam(url)
            .request(Secret::new("test".into()))
            .await
            .err()
            .unwrap();
        assert!(error.message.contains("64 KiB"));
        request.await.unwrap();
    }
}
#[tokio::test]
async fn cancellation_covers_waiting_response_and_precedes_bootstrap() {
    let cancel = Cancellation::new();
    cancel.cancel();
    let prepared = Prepared::new(&config(), "instance", None).unwrap();
    assert_eq!(
        prepared
            .resolve(&BlockingPool::new(), &cancel)
            .await
            .err()
            .unwrap()
            .code,
        130
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = Url::parse(&format!(
        "http://{}/v1/secret/data/team/token",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let cancel = Cancellation::new();
    let worker_cancel = cancel.clone();
    let worker = tokio::spawn(async move {
        seam(url)
            .request_cancellable(Secret::new("test".into()), &worker_cancel)
            .await
    });
    let (_connection, _) = listener.accept().await.unwrap();
    cancel.cancel();
    let error = tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(error.code, 130);
}
#[test]
fn production_rejects_http_and_bad_ca_before_bootstrap() {
    let mut value = serde_json::to_value(config()).unwrap();
    value["origin"] = serde_json::json!("http://localhost:1234");
    let config: CredentialResolver = serde_json::from_value(value).unwrap();
    assert!(Prepared::new(&config, "instance", None).is_err());
    let network = NetworkContext {
        ca_bundle_pem: Some(
            "-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n".into(),
        ),
        ..Default::default()
    };
    assert!(Prepared::new(&super::tests::config(), "instance", Some(&network)).is_err());
}
#[tokio::test]
async fn body_read_deadline_is_typed_and_response_is_not_reflected() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = Url::parse(&format!(
        "http://{}/v1/secret/data/team/token",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let worker = tokio::spawn(async move {
        let mut prepared = seam(url);
        prepared.client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();
        prepared.request(Secret::new("test".into())).await
    });
    let (mut connection, _) = listener.accept().await.unwrap();
    connection
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nSECRET_SENTINEL")
        .await
        .unwrap();
    let error = tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap()
        .err()
        .unwrap();
    assert_eq!(
        error.diagnostic,
        Some(crate::provider_diagnostics::Code::DeadlineExceeded)
    );
    assert!(!error.message.contains("SENTINEL"));
}

#[tokio::test]
async fn connect_get_uses_bearer_and_exact_item_without_other_fields() {
    let (mut url, request)=server(response("200 OK",r#"{"id":"zyxwvutsrqponmlkjihgfedcba","vault":{"id":"abcdefghijklmnopqrstuvwxyz"},"version":3,"fields":[{"id":"password","value":"selected-sentinel"},{"id":"unrelated","value":"other-sentinel"}]}"#)).await;
    url.set_path("/v1/vaults/abcdefghijklmnopqrstuvwxyz/items/zyxwvutsrqponmlkjihgfedcba");
    let mut prepared = seam(url);
    prepared.config=serde_json::from_value(serde_json::json!({"type":"1password_connect","version":1,"origin":"https://connect.example","vault":"abcdefghijklmnopqrstuvwxyz","item":"zyxwvutsrqponmlkjihgfedcba","field":"password","bootstrap":"env://BOOTSTRAP"})).unwrap();
    let value = prepared
        .request(Secret::new("connect-bootstrap".into()))
        .await
        .unwrap();
    assert_eq!(value.secret.expose(), "selected-sentinel");
    let request = request.await.unwrap();
    assert!(request.starts_with(
        "GET /v1/vaults/abcdefghijklmnopqrstuvwxyz/items/zyxwvutsrqponmlkjihgfedcba HTTP/1.1\r\n"
    ));
    assert!(request.contains("authorization: Bearer connect-bootstrap\r\n"));
    assert!(!request.contains("x-vault-token"));
}

#[tokio::test]
async fn selected_field_cannot_forward_the_host_only_bootstrap() {
    for selected in ["private-bootstrap", "prefix-private-bootstrap-suffix"] {
        let body=serde_json::json!({"data":{"data":{"token":selected},"metadata":{"version":1,"destroyed":false,"deletion_time":""}}}).to_string();
        let (url, request) = server(response("200 OK", &body)).await;
        let error = seam(url)
            .request(Secret::new("private-bootstrap".into()))
            .await
            .err()
            .unwrap();
        assert!(error.message.contains("reflects"));
        assert!(!error.message.contains("private-bootstrap"));
        request.await.unwrap();
    }
}
