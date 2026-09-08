// SPDX-License-Identifier: MIT
//! Host-owned, ephemeral OAuth code flow. No provider process sees these values.
mod callback;
mod token;
use crate::cancellation::Cancellation;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use permesh_provider_sdk::browser_auth::{BrowserAuthSpec, endpoint};
use permesh_secrets::Secret;
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use zeroize::Zeroizing;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowError {
    Configuration,
    Callback,
    Denied,
    Token,
    Timeout,
    Cancelled,
    Browser,
}
impl FlowError {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Configuration => {
                "Browser authentication declaration or OAuth client settings are invalid."
            }
            Self::Callback => "No valid OAuth callback was received.",
            Self::Denied => "Browser authorization was denied. No credential was stored.",
            Self::Token => {
                "OAuth token exchange failed. Check the configured OAuth client and granted permissions; no credential was stored."
            }
            Self::Timeout => {
                "Browser authentication exceeded its deadline. No credential was stored."
            }
            Self::Cancelled => "Cancelled",
            Self::Browser => {
                "Cannot open the system browser. Retry with --browser --no-open to open the authorization URL manually."
            }
        }
    }
}
fn random_secret() -> Result<Secret, FlowError> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    getrandom::fill(&mut *bytes).map_err(|_| FlowError::Configuration)?;
    Ok(Secret::new(URL_SAFE_NO_PAD.encode(*bytes)))
}
pub(crate) struct Pending {
    listener: TcpListener,
    authorization: url::Url,
    token_endpoint: url::Url,
    redirect_uri: String,
    client_id: String,
    state: Secret,
    verifier: Secret,
    scopes: Vec<String>,
    deadline: tokio::time::Instant,
}
impl Pending {
    pub(crate) async fn begin(spec: BrowserAuthSpec, client_id: String) -> Result<Self, FlowError> {
        spec.validate().map_err(|_| FlowError::Configuration)?;
        if client_id.is_empty()
            || client_id.len() > 1024
            || !client_id.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(FlowError::Configuration);
        }
        let token_endpoint =
            endpoint(&spec.token_endpoint).map_err(|_| FlowError::Configuration)?;
        let mut authorization =
            endpoint(&spec.authorization_endpoint).map_err(|_| FlowError::Configuration)?;
        let state = random_secret()?;
        let verifier = random_secret()?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.expose()));
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| FlowError::Callback)?;
        let redirect_uri = format!(
            "http://127.0.0.1:{}/oauth/callback",
            listener
                .local_addr()
                .map_err(|_| FlowError::Callback)?
                .port()
        );
        authorization
            .query_pairs_mut()
            .extend_pairs([
                ("response_type", "code"),
                ("client_id", client_id.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("scope", &spec.scopes.join(" ")),
                ("state", state.expose()),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", "S256"),
            ])
            .extend_pairs(spec.authorization_parameters.iter());
        Ok(Self {
            listener,
            authorization,
            token_endpoint,
            redirect_uri,
            client_id,
            state,
            verifier,
            scopes: spec.scopes,
            deadline: tokio::time::Instant::now() + Duration::from_secs(300),
        })
    }
    pub(crate) fn url(&self) -> &url::Url {
        &self.authorization
    }
    pub(crate) async fn finish(
        self,
        client_secret: Option<&Secret>,
        cancel: &Cancellation,
    ) -> Result<Secret, FlowError> {
        tokio::select! {biased;()=cancel.cancelled()=>Err(FlowError::Cancelled),result=tokio::time::timeout_at(self.deadline,self.complete(client_secret))=>result.map_err(|_|FlowError::Timeout)?}
    }
    pub(crate) fn validate_secret(&self, client_secret: Option<&Secret>) -> Result<(), FlowError> {
        if client_secret.is_some_and(|s| {
            s.expose().is_empty()
                || s.expose().len() > 16 * 1024
                || [self.authorization.as_str(), self.token_endpoint.as_str()]
                    .iter()
                    .any(|url| {
                        url.contains(s.expose())
                            || url::form_urlencoded::parse(url.as_bytes())
                                .any(|(k, v)| k.contains(s.expose()) || v.contains(s.expose()))
                    })
        }) {
            return Err(FlowError::Configuration);
        }
        Ok(())
    }
    async fn complete(&self, client_secret: Option<&Secret>) -> Result<Secret, FlowError> {
        self.validate_secret(client_secret)?;
        let host = format!(
            "127.0.0.1:{}",
            self.listener
                .local_addr()
                .map_err(|_| FlowError::Callback)?
                .port()
        );
        let mut accepted = None;
        for _ in 0..16 {
            let (mut socket, address) = self
                .listener
                .accept()
                .await
                .map_err(|_| FlowError::Callback)?;
            if !address.ip().is_loopback() {
                continue;
            }
            let result = tokio::time::timeout(Duration::from_secs(5), async {
                let mut bytes = Zeroizing::new(Vec::with_capacity(16 * 1024));
                while !bytes.ends_with(b"\r\n\r\n") {
                    let mut chunk = Zeroizing::new([0; 1024]);
                    let remaining = 16 * 1024 - bytes.len();
                    if remaining == 0 {
                        return Err(FlowError::Callback);
                    }
                    let n = socket
                        .read(&mut chunk[..remaining.min(1024)])
                        .await
                        .map_err(|_| FlowError::Callback)?;
                    if n == 0 {
                        return Err(FlowError::Callback);
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                }
                callback::parse(&bytes, &host, self.state.expose())
            })
            .await
            .unwrap_or(Err(FlowError::Callback));
            let (status, body) = if result.is_ok() {
                (
                    "200 OK",
                    "Authorization response received. Return to Permesh.\n",
                )
            } else {
                ("400 Bad Request", "Invalid OAuth callback.\n")
            };
            let reply = format!(
                "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Type: text/plain\r\nCache-Control: no-store\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            // Browser response delivery is best effort: an already-closed browser
            // socket cannot revoke a valid, state-bound authorization callback.
            match tokio::time::timeout(Duration::from_secs(1), socket.write_all(reply.as_bytes()))
                .await
            {
                Ok(Ok(())) | Ok(Err(_)) | Err(_) => {}
            }
            match result {
                Ok(code) => {
                    accepted = Some(code);
                    break;
                }
                Err(FlowError::Denied) => return Err(FlowError::Denied),
                Err(_) => {}
            }
        }
        let code = accepted.ok_or(FlowError::Callback)?;
        let mut fields = vec![
            ("grant_type", "authorization_code"),
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("code", code.expose()),
            ("code_verifier", self.verifier.expose()),
        ];
        let mut protected = vec![code.expose(), self.verifier.expose(), self.state.expose()];
        if let Some(secret) = client_secret {
            fields.push(("client_secret", secret.expose()));
            protected.push(secret.expose());
        }
        token::exchange(
            self.token_endpoint.clone(),
            &fields,
            &self.scopes,
            &protected,
        )
        .await
    }
}
#[cfg(test)]
mod tests;

/// Fixed native opener, argument vector only; never consult BROWSER or a shell.
pub(crate) async fn open(url: &url::Url, cancel: &Cancellation) -> Result<(), FlowError> {
    #[cfg(target_os = "macos")]
    let mut command = tokio::process::Command::new("/usr/bin/open");
    #[cfg(target_os = "linux")]
    let mut command = tokio::process::Command::new("/usr/bin/xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let root = std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or(FlowError::Browser)?;
        // Explorer uses the registered HTTPS handler without shell parsing or
        // rundll32's DLL search path. SystemRoot supports non-default installs.
        let mut c = tokio::process::Command::new(root.join("explorer.exe"));
        c.current_dir(&root);
        c
    };
    #[cfg(unix)]
    command.current_dir("/");
    command
        .arg(url.as_str())
        .env_remove("BROWSER")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|_| FlowError::Browser)?;
    let result = tokio::select! {biased;()=cancel.cancelled()=>Err(FlowError::Cancelled),result=tokio::time::timeout(Duration::from_secs(10),child.wait())=>match result{Ok(Ok(status)) if status.success()=>Ok(()),_=>Err(FlowError::Browser)}};
    if result.is_err() {
        match child.kill().await {
            Ok(()) | Err(_) => {}
        }
    }
    result
}
