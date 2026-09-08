// SPDX-License-Identifier: MIT
use super::*;
use serde::Deserialize;
const MAX_BODY: usize = 64 * 1024;
fn form(fields: &[(&str, &str)]) -> Result<Zeroizing<Vec<u8>>, FlowError> {
    if fields.len() > 8
        || fields
            .iter()
            .any(|(k, v)| k.len() > 64 || v.len() > 16 * 1024)
    {
        return Err(FlowError::Token);
    }
    let mut body = Zeroizing::new(Vec::with_capacity(
        fields.iter().map(|(k, v)| k.len() + 2 + 3 * v.len()).sum(),
    ));
    const HEX: &[u8] = b"0123456789ABCDEF";
    for (index, (key, value)) in fields.iter().enumerate() {
        if index > 0 {
            body.push(b'&');
        }
        body.extend_from_slice(key.as_bytes());
        body.push(b'=');
        for b in value.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                body.push(b);
            } else {
                body.extend_from_slice(&[b'%', HEX[(b >> 4) as usize], HEX[(b & 15) as usize]]);
            }
        }
    }
    Ok(body)
}
#[derive(Deserialize)]
struct Response {
    access_token: Zeroizing<String>,
    refresh_token: Zeroizing<String>,
    token_type: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<Zeroizing<String>>,
}
fn valid_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 16 * 1024 && value.bytes().all(|b| b.is_ascii_graphic())
}
pub(super) async fn exchange(
    endpoint: url::Url,
    fields: &[(&str, &str)],
    scopes: &[String],
    protected: &[&str],
) -> Result<Secret, FlowError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| FlowError::Token)?;
    tokio::time::timeout(Duration::from_secs(15), async {
        let body = bytes::Bytes::from_owner(form(fields)?);
        let mut response = client
            .post(endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| FlowError::Token)?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > MAX_BODY as u64)
        {
            return Err(FlowError::Token);
        }
        let mut buffer = Zeroizing::new(Vec::with_capacity(MAX_BODY));
        while let Some(chunk) = response.chunk().await.map_err(|_| FlowError::Token)? {
            if chunk.len() > MAX_BODY - buffer.len() {
                return Err(FlowError::Token);
            }
            buffer.extend_from_slice(&chunk);
        }
        let mut result: Response = serde_json::from_slice(&buffer).map_err(|_| FlowError::Token)?;
        if !valid_token(&result.access_token)
            || !valid_token(&result.refresh_token)
            || !result.token_type.eq_ignore_ascii_case("Bearer")
            || result.expires_in == Some(0)
            || protected
                .iter()
                .any(|v| !v.is_empty() && result.refresh_token.contains(v))
        {
            return Err(FlowError::Token);
        }
        if let Some(scope) = &result.scope {
            let granted: Vec<_> = scope.split(' ').collect();
            if granted.is_empty()
                || granted
                    .iter()
                    .any(|v| v.is_empty() || !scopes.iter().any(|s| s == v))
            {
                return Err(FlowError::Token);
            }
        }
        Ok(Secret::new(std::mem::take(&mut *result.refresh_token)))
    })
    .await
    .map_err(|_| FlowError::Timeout)?
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(super) mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    pub(crate) async fn mock(
        status: u16,
        headers: &str,
        body: &str,
    ) -> (url::Url, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url =
            url::Url::parse(&format!("http://{}/token", listener.local_addr().unwrap())).unwrap();
        let reply = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
            body.len()
        );
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buf = [0; 4096];
                let n = socket.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&bytes);
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| "0".into())
                        .parse::<usize>()
                        .unwrap();
                    if body.len() >= length {
                        break;
                    }
                }
            }
            let _ = socket.write_all(reply.as_bytes()).await;
            String::from_utf8(bytes).unwrap()
        });
        (url, task)
    }
    #[tokio::test]
    async fn exchanges_bound_form_and_returns_only_refresh_credential() {
        let(url,task)=mock(200,"",r#"{"access_token":"ACCESS","token_type":"Bearer","refresh_token":"REFRESH","expires_in":3600,"scope":"read"}"#).await;
        let secret = exchange(
            url,
            &[
                ("code", "CODE&value"),
                ("code_verifier", "VERIFIER"),
                ("client_secret", "CLIENT"),
            ],
            &["read".into()],
            &["CODE&value", "VERIFIER", "CLIENT"],
        )
        .await
        .unwrap();
        assert_eq!(secret.expose(), "REFRESH");
        let request = task.await.unwrap();
        assert!(request.contains("code=CODE%26value"));
        assert!(!format!("{secret:?}").contains("REFRESH"));
    }
    #[tokio::test]
    async fn failures_reflections_scope_widening_and_redirects_are_rejected_without_echo() {
        for(status,body)in [(400,"SECRET error".into()),(200,r#"{"access_token":"ACCESS","token_type":"Bearer","refresh_token":"prefixSECRET"}"#.into()),(200,r#"{"access_token":"ACCESS","token_type":"Bearer","refresh_token":"REFRESH","scope":"admin"}"#.into()),(200,r#"{"access_token":"ACCESS","token_type":"Bearer","refresh_token":"R","refresh_token":"SECOND"}"#.into()),(200,"S".repeat(65537))] {
     let(url,task)=mock(status,"",&body).await;let err=exchange(url,&[("code","SECRET")],&["read".into()],&["SECRET"]).await.unwrap_err();assert!(!format!("{err:?}").contains("SECRET"));task.await.unwrap();
   }
        let (target, mut destination) = mock(
            200,
            "",
            r#"{"access_token":"ACCESS","token_type":"Bearer","refresh_token":"REFRESH"}"#,
        )
        .await;
        let (url, task) = mock(307, &format!("Location: {target}\r\n"), "").await;
        assert!(
            exchange(url, &[("code", "SECRET")], &["read".into()], &["SECRET"])
                .await
                .is_err()
        );
        task.await.unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), &mut destination)
                .await
                .is_err()
        );
        destination.abort();
    }
}
