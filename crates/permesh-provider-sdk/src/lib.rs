// SPDX-License-Identifier: MIT OR Apache-2.0
//! Read-only adapter boundary. Providers never print or correlate identities.
use permesh_core::Snapshot;
use serde::{Deserialize, Serialize};
use std::{future::Future, pin::Pin};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Accounts,
    Identities,
    Resources,
    Groups,
    Memberships,
    Grants,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub kind: String,
    pub capabilities: Vec<Capability>,
}
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
#[error("{message}")]
pub struct ProviderError {
    pub code: String,
    pub message: String,
}
impl ProviderError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Health {
    pub message: String,
    pub limitations: Vec<String>,
}
pub type ProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send + 'a>>;
pub trait Provider: Send + Sync {
    fn metadata(&self) -> Metadata;
    fn check(&self) -> ProviderFuture<'_, Health>;
    fn discover(&self) -> ProviderFuture<'_, Snapshot>;
}
/// Shared contract used by every adapter and by the application before queries.
pub fn validate_snapshot(snapshot: &Snapshot) -> Result<(), ProviderError> {
    snapshot
        .validate()
        .map_err(|e| ProviderError::new("invalid_snapshot", e.to_string()))
}
