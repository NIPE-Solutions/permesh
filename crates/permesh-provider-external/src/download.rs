// SPDX-License-Identifier: MIT
//! Explicit downloads from the official catalog and GitHub release assets.
use crate::{
    DistributionError,
    catalog::{self, Catalog, Release},
};
use reqwest::{Client, Url, header};
use sha2::{Digest, Sha256};
use std::time::Duration;

pub const CATALOG_URL: &str =
    "https://raw.githubusercontent.com/NIPE-Solutions/permesh-providers/main/catalog/v1.json";

fn client() -> Result<Client, DistributionError> {
    Client::builder()
        .no_proxy()
        .referer(false)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| DistributionError::Network)
}

fn allowed_asset_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.fragment().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

// Kept private: production callers can only start at the fixed catalog URL or
// a validated Release-derived URL. Unit tests inject a local server here.
async fn read(
    client: &Client,
    initial: String,
    limit: usize,
    archive_redirects: bool,
) -> Result<Vec<u8>, DistributionError> {
    let mut url = Url::parse(&initial).map_err(|_| DistributionError::Network)?;
    for redirects in 0..=5 {
        let mut response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|_| DistributionError::Network)?;
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            if !archive_redirects || redirects == 5 {
                return Err(DistributionError::Network);
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or(DistributionError::Network)?;
            let next = url.join(location).map_err(|_| DistributionError::Network)?;
            if !allowed_asset_url(&next) {
                return Err(DistributionError::Network);
            }
            url = next;
            continue;
        }
        if response.status() != reqwest::StatusCode::OK
            || response
                .headers()
                .get_all(header::CONTENT_ENCODING)
                .iter()
                .any(|value| value.as_bytes() != b"identity")
        {
            return Err(DistributionError::Network);
        }
        if response
            .content_length()
            .is_some_and(|size| size > limit as u64)
        {
            return Err(DistributionError::Integrity);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| DistributionError::Network)?
        {
            if chunk.len() > limit.saturating_sub(bytes.len()) {
                return Err(DistributionError::Integrity);
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok(bytes);
    }
    Err(DistributionError::Network)
}

fn verify(bytes: &[u8], size: u64, digest: &str) -> Result<(), DistributionError> {
    let actual: String = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if bytes.len() as u64 != size || actual != digest {
        return Err(DistributionError::Integrity);
    }
    Ok(())
}

/// Fetch the fixed catalog within a single 30-second deadline. Dropping the
/// future cancels its request; no background download task is spawned.
pub async fn fetch_catalog() -> Result<Catalog, DistributionError> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let bytes = read(
            &client()?,
            CATALOG_URL.into(),
            catalog::MAX_CATALOG_BYTES,
            false,
        )
        .await?;
        catalog::parse(&bytes)
    })
    .await
    .map_err(|_| DistributionError::Network)?
}

/// Download and verify the exact catalog archive within 120 seconds, including
/// all redirect hops and streamed body bytes.
pub async fn fetch_archive(release: &Release) -> Result<Vec<u8>, DistributionError> {
    let url = release.asset_url()?;
    tokio::time::timeout(Duration::from_secs(120), async {
        let limit =
            usize::try_from(release.archive_size).map_err(|_| DistributionError::Integrity)?;
        let bytes = read(&client()?, url, limit, true).await?;
        verify(&bytes, release.archive_size, &release.archive_sha256)?;
        Ok(bytes)
    })
    .await
    .map_err(|_| DistributionError::Network)?
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn mock(response: &'static [u8]) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/test", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let count = socket.read(&mut request).await.unwrap();
            request.truncate(count);
            socket.write_all(response).await.unwrap();
            request
        });
        (url, task)
    }

    #[tokio::test]
    async fn accepts_bounded_body_without_auth_or_referer() {
        let (url, task) =
            mock(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata").await;
        let bytes = read(&client().unwrap(), url, 4, false).await.unwrap();
        assert_eq!(bytes, b"data");
        let request = String::from_utf8(task.await.unwrap())
            .unwrap()
            .to_ascii_lowercase();
        for forbidden in [
            "authorization:",
            "proxy-authorization:",
            "referer:",
            "accept-encoding:",
        ] {
            assert!(!request.contains(forbidden));
        }
    }

    #[tokio::test]
    async fn rejects_declared_and_streamed_oversize_truncation_compression_and_errors() {
        for response in [
            &b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\n12345"[..],
            &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n12345\r\n0\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n12"[..],
            &b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nContent-Encoding: gzip\r\nConnection: close\r\n\r\ndata"[..],
            &b"HTTP/1.1 403 Forbidden\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndata"[..],
        ] {
            let (url,task)=mock(response).await;
            assert!(read(&client().unwrap(),url,4,false).await.is_err());
            task.await.unwrap();
        }
    }

    #[tokio::test]
    async fn rejects_metadata_redirects_and_archive_unapproved_destinations() {
        for redirects in [false, true] {
            let (url,task)=mock(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            assert!(read(&client().unwrap(), url, 4, redirects).await.is_err());
            task.await.unwrap();
        }
    }

    #[test]
    fn exact_https_redirect_allowlist_and_archive_integrity() {
        for url in [
            "https://github.com/a",
            "https://release-assets.githubusercontent.com/a?sig=test",
            "https://objects.githubusercontent.com/a",
        ] {
            assert!(allowed_asset_url(&reqwest::Url::parse(url).unwrap()));
        }
        for url in [
            "http://github.com/a",
            "https://github.com.evil.invalid/a",
            "https://user:pass@github.com/a",
            "https://github.com:444/a",
            "https://raw.githubusercontent.com/a",
            "https://github.com/a#fragment",
        ] {
            assert!(!allowed_asset_url(&reqwest::Url::parse(url).unwrap()));
        }
        let digest: String = Sha256::digest(b"data")
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert!(verify(b"data", 4, &digest).is_ok());
        assert!(verify(b"data", 3, &digest).is_err());
        assert!(verify(b"evil", 4, &digest).is_err());
    }

    #[tokio::test]
    async fn stalled_request_is_cancelled_by_dropping_future() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let client = client().unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                read(&client, url, 4, false)
            )
            .await
            .is_err()
        );
    }
}
