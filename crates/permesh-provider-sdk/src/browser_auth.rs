// SPDX-License-Identifier: MIT
//! Declarative OAuth authorization-code login with PKCE. Contains no credentials.
use crate::setup::Condition;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserAuthSpec {
    pub schema_version: u32,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub scopes: Vec<String>,
    pub client_id_field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret_slot: Option<String>,
    pub refresh_token_slot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    pub authorization_parameters: BTreeMap<String, String>,
}
impl std::fmt::Debug for BrowserAuthSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserAuthSpec([REDACTED])")
    }
}
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("invalid or unsupported browser authentication declaration")]
pub struct BrowserAuthError;
fn identifier(v: &str) -> bool {
    (1..=64).contains(&v.len())
        && v.as_bytes()[0].is_ascii_alphabetic()
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
/// Endpoint declarations never contain credentials, query parameters, alternate
/// ports or local IP addresses. Host authentication does not follow redirects.
pub fn endpoint(value: &str) -> Result<url::Url, BrowserAuthError> {
    if value.contains('\\')
        || value
            .split("/")
            .nth(2)
            .is_some_and(|authority| authority.contains('@'))
        || value.len() > 2048
        || !value.is_ascii()
        || value
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return Err(BrowserAuthError);
    }
    let url = url::Url::parse(value).map_err(|_| BrowserAuthError)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.host(),Some(url::Host::Domain(host)) if host.contains('.') && host!="localhost" && !host.ends_with(".localhost"))
    {
        return Err(BrowserAuthError);
    }
    Ok(url)
}
impl BrowserAuthSpec {
    pub fn validate(&self) -> Result<(), BrowserAuthError> {
        if self.schema_version != 1
            || !identifier(&self.client_id_field)
            || !identifier(&self.refresh_token_slot)
            || self
                .client_secret_slot
                .as_ref()
                .is_some_and(|v| !identifier(v) || v == &self.refresh_token_slot)
        {
            return Err(BrowserAuthError);
        }
        endpoint(&self.authorization_endpoint)?;
        endpoint(&self.token_endpoint)?;
        if self.scopes.is_empty() || self.scopes.len() > 16 {
            return Err(BrowserAuthError);
        }
        let mut scopes = BTreeSet::new();
        for scope in &self.scopes {
            if scope.is_empty()
                || scope.len() > 512
                || !scope
                    .bytes()
                    .all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'\\'))
                || !scopes.insert(scope)
            {
                return Err(BrowserAuthError);
            }
        }
        if self.when.as_ref().is_some_and(|condition| !identifier(&condition.field)
                || !matches!(&condition.equals,serde_json::Value::String(v) if !v.is_empty() && v.len()<=128 && v.bytes().all(|b|b.is_ascii_graphic()))) {
            return Err(BrowserAuthError);
        }
        // This first version supports only consent/offline hints. Arbitrary
        // extras could change audience, requested privileges or callback binding.
        for (key, value) in &self.authorization_parameters {
            if !matches!(
                (key.as_str(), value.as_str()),
                ("access_type", "offline") | ("prompt", "consent")
            ) {
                return Err(BrowserAuthError);
            }
        }
        Ok(())
    }
}
