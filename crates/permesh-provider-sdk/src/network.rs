// SPDX-License-Identifier: MIT
//! Explicit non-secret provider transport context. No filesystem or environment access.
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::BTreeSet, fmt, net::IpAddr};
use url::Url;

pub const MAX_CA_BYTES: usize = 256 * 1024;
const MAX_CERTIFICATES: usize = 64;
const MAX_PROXY_BYTES: usize = 2048;
const MAX_BYPASS_HOSTS: usize = 64;

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkContext {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub https_proxy: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no_proxy: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub ca_bundle_pem: Option<String>,
}
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
impl fmt::Debug for NetworkContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NetworkContext([REDACTED])")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid explicit provider network context")]
pub struct NetworkError;

impl NetworkContext {
    /// Validate bounded syntax. TLS consumers must additionally parse every DER
    /// certificate; this framing check never claims that a certificate is trusted.
    pub fn validate(&self) -> Result<(), NetworkError> {
        if self.https_proxy.is_none() && self.ca_bundle_pem.is_none() {
            return Err(NetworkError);
        }
        if let Some(proxy) = &self.https_proxy {
            validate_proxy(proxy)?;
        }
        if self.no_proxy.len() > MAX_BYPASS_HOSTS
            || (!self.no_proxy.is_empty() && self.https_proxy.is_none())
        {
            return Err(NetworkError);
        }
        let mut seen = BTreeSet::new();
        for host in &self.no_proxy {
            if !bypass_host(host) || !seen.insert(host.to_ascii_lowercase()) {
                return Err(NetworkError);
            }
        }
        if let Some(pem) = &self.ca_bundle_pem {
            validate_ca_pem(pem)?;
        }
        Ok(())
    }
}
fn validate_proxy(value: &str) -> Result<(), NetworkError> {
    if value.len() > MAX_PROXY_BYTES
        || !value.is_ascii()
        || value
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        || value.contains(['@', '\\'])
        || !(value.starts_with("http://") || value.starts_with("https://"))
    {
        return Err(NetworkError);
    }
    // Check the original authority before URL normalization can remove dot
    // segments or repair extra slashes after the scheme.
    let authority = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .ok_or(NetworkError)?;
    let authority = authority.strip_suffix('/').unwrap_or(authority);
    if authority.is_empty() || authority.contains('/') {
        return Err(NetworkError);
    }
    let url = Url::parse(value).map_err(|_| NetworkError)?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.port() == Some(0)
    {
        return Err(NetworkError);
    }
    Ok(())
}
fn bypass_host(value: &str) -> bool {
    if value.is_empty() || value.len() > 253 || !value.is_ascii() {
        return false;
    }
    if value == "*" || value.parse::<IpAddr>().is_ok() {
        return true;
    }
    let domain = value.strip_prefix('.').unwrap_or(value);
    domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}
/// Reject arbitrary file contents and private keys before a host delivers a CA
/// bundle to a provider. Only PEM CERTIFICATE blocks and whitespace are accepted.
pub fn validate_ca_pem(pem: &str) -> Result<(), NetworkError> {
    if pem.is_empty() || pem.len() > MAX_CA_BYTES || !pem.is_ascii() {
        return Err(NetworkError);
    }
    let mut body = None::<String>;
    let mut count = 0;
    for line in pem.lines() {
        let line = line.trim();
        match (body.as_mut(), line) {
            (None, "") => (),
            (None, "-----BEGIN CERTIFICATE-----") => body = Some(String::new()),
            (Some(encoded), "-----END CERTIFICATE-----") => {
                let data = encoded.trim_end_matches('=');
                let padding = encoded.len() - data.len();
                if encoded.is_empty()
                    || encoded.len() % 4 != 0
                    || padding > 2
                    || data
                        .bytes()
                        .any(|b| !(b.is_ascii_alphanumeric() || b == b'+' || b == b'/'))
                {
                    return Err(NetworkError);
                }
                count += 1;
                if count > MAX_CERTIFICATES {
                    return Err(NetworkError);
                }
                body = None;
            }
            (Some(encoded), content)
                if !content.is_empty()
                    && content
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')) =>
            {
                encoded.push_str(content)
            }
            _ => return Err(NetworkError),
        }
    }
    if body.is_some() || count == 0 {
        return Err(NetworkError);
    }
    Ok(())
}
