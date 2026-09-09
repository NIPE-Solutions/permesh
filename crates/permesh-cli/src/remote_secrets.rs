// SPDX-License-Identifier: MIT
//! Host-owned exact-locator HTTPS reads. No source response or token is diagnostic text.
use crate::{blocking::BlockingPool, cancellation::Cancellation, error::AppError};
use permesh_config::CredentialResolver;
use permesh_provider_sdk::network::NetworkContext;
use permesh_secrets::{Secret, SecretRef, SecretResolver};
use reqwest::{Client, Url};
use std::time::Duration;
use zeroize::Zeroizing;
const MAX_RESPONSE: usize = 65_536;
const MAX_SECRET: usize = 16_384;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
#[path = "remote_secrets_decode.rs"]
mod decode;
/// Metadata never contains a secret value and is not inserted into access records.
pub(crate) struct Resolved {
    pub secret: Secret,
    pub version: u64,
    pub expires_at: Option<time::OffsetDateTime>,
}
#[derive(Clone)]
pub(crate) struct Prepared {
    config: CredentialResolver,
    client: Client,
    url: Url,
}
fn error(message: &'static str) -> AppError {
    AppError::new(3, message).diagnostic(crate::provider_diagnostics::Code::CredentialUnavailable)
}
fn deadline() -> AppError {
    error("Remote credential resolution exceeded its 15-second deadline")
        .diagnostic(crate::provider_diagnostics::Code::DeadlineExceeded)
}
fn invalid() -> AppError {
    AppError::input("Invalid remote credential resolver or network configuration")
        .diagnostic(crate::provider_diagnostics::Code::InvalidConfiguration)
}
impl Prepared {
    pub(crate) fn new(
        config: &CredentialResolver,
        instance: &str,
        network: Option<&NetworkContext>,
    ) -> Result<Self, AppError> {
        config.validate(instance).map_err(|_| invalid())?;
        let url = config.target_url().map_err(|_| invalid())?;
        let mut builder = Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(REQUEST_TIMEOUT);
        if let Some(network) = network {
            network.validate().map_err(|_| invalid())?;
            if let Some(proxy) = &network.https_proxy {
                builder = builder.proxy(
                    reqwest::Proxy::https(proxy)
                        .map_err(|_| invalid())?
                        .no_proxy(reqwest::NoProxy::from_string(&network.no_proxy.join(","))),
                );
            }
            if let Some(pem) = &network.ca_bundle_pem {
                let certs =
                    reqwest::Certificate::from_pem_bundle(pem.as_bytes()).map_err(|_| invalid())?;
                if certs.is_empty() {
                    return Err(invalid());
                }
                Client::builder()
                    .no_proxy()
                    .tls_certs_only(certs.clone())
                    .build()
                    .map_err(|_| invalid())?;
                for certificate in certs {
                    builder = builder.add_root_certificate(certificate);
                }
            }
        }
        Ok(Self {
            config: config.clone(),
            client: builder.build().map_err(|_| invalid())?,
            url,
        })
    }
    pub(crate) async fn resolve(
        self,
        pool: &BlockingPool,
        cancel: &Cancellation,
    ) -> Result<Resolved, AppError> {
        if cancel.is_cancelled() {
            return Err(AppError::new(130, "Cancelled"));
        }
        let bootstrap = SecretRef::parse(self.config.bootstrap()).map_err(|_| invalid())?;
        let token=tokio::select!{biased;()=cancel.cancelled()=>return Err(AppError::new(130,"Cancelled")), result=pool.run(move||SecretResolver.resolve(&bootstrap))=>result?}.map_err(|_|error("Remote resolver bootstrap unavailable; set its explicit environment reference or same-instance keychain entry"))?;
        if cancel.is_cancelled() {
            return Err(AppError::new(130, "Cancelled"));
        }
        self.request_cancellable(token, cancel).await
    }
    async fn request_cancellable(
        self,
        token: Secret,
        cancel: &Cancellation,
    ) -> Result<Resolved, AppError> {
        tokio::select! {biased;()=cancel.cancelled()=>Err(AppError::new(130,"Cancelled")),result=tokio::time::timeout(REQUEST_TIMEOUT,self.request(token))=>result.map_err(|_|deadline())?}
    }
    async fn request(self, token: Secret) -> Result<Resolved, AppError> {
        if token.expose().is_empty() || token.expose().len() > MAX_SECRET {
            return Err(error("Remote resolver bootstrap has an invalid size"));
        }
        let mut request = self.client.get(self.url.clone());
        let token_header = match &self.config {
            CredentialResolver::OnePasswordConnect { .. } => {
                Zeroizing::new(format!("Bearer {}", token.expose()))
            }
            _ => Zeroizing::new(token.expose().to_owned()),
        };
        let mut header = reqwest::header::HeaderValue::from_str(&token_header).map_err(|_| {
            error("Remote resolver bootstrap is not valid authentication header text")
        })?;
        header.set_sensitive(true);
        request = request.header(
            if matches!(self.config, CredentialResolver::OnePasswordConnect { .. }) {
                "authorization"
            } else {
                "x-vault-token"
            },
            header,
        );
        let mut response=request.send().await.map_err(|failure|if failure.is_timeout(){deadline()}else{error("Remote credential endpoint unavailable; check approved HTTPS, proxy, CA and bootstrap settings")})?;
        if response.url() != &self.url {
            return Err(error(
                "Remote credential response changed the approved destination",
            ));
        }
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 | 403 => error(
                    "Remote credential access denied; check the bootstrap token and exact read permissions",
                ),
                404 => error("Remote credential item, path or version is unavailable"),
                300..=399 => error("Remote credential redirects are not permitted"),
                _ => error("Remote credential service rejected the exact read request"),
            });
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_RESPONSE as u64)
        {
            return Err(error("Remote credential response exceeds the 64 KiB limit"));
        }
        // Reserve the bound before receiving secret bytes so growth cannot leave old plaintext allocations.
        let mut body = Zeroizing::new(Vec::with_capacity(MAX_RESPONSE));
        while let Some(chunk) = response.chunk().await.map_err(|failure| {
            if failure.is_timeout() {
                deadline()
            } else {
                error("Remote credential response could not be read")
            }
        })? {
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE {
                return Err(error("Remote credential response exceeds the 64 KiB limit"));
            }
            body.extend_from_slice(&chunk);
        }
        let resolved = decode::decode(&self.config, &body, time::OffsetDateTime::now_utc())?;
        if resolved.secret.expose().contains(token.expose()) {
            return Err(error(
                "Remote credential field reflects the host-only bootstrap token",
            ));
        }
        Ok(resolved)
    }
}

#[cfg(test)]
#[path = "remote_secrets_tests.rs"]
mod tests;
