// SPDX-License-Identifier: MIT
//! Opt-in negotiated protocol version 1, independent of legacy operation-specific drafts.
mod mapping;
pub mod records;
mod session;
mod wire;
use crate::ProtocolError;
use serde::{Deserialize, Serialize};
pub use session::{DiscoveryDecoder, HealthDecoder};
/// The only version supported by this negotiated contract. Never downgrade.
pub const PROTOCOL_VERSION: u32 = 1;
/// Maximum unique advertised operations. Optional unknown operations never grant capabilities.
pub const MAX_OPERATIONS: usize = 16;
/// Operation names are 1–64 ASCII bytes, start with a letter, and contain only
/// letters, digits, `_` and `-`. Matching selected operations is case-sensitive.
pub const MAX_OPERATION_NAME_BYTES: usize = 64;
/// Optional provider codes contain 1–64 lowercase ASCII letters, digits, `.`,
/// `_` or `-`. Codes are validated but never included in errors or results.
pub const MAX_PROVIDER_CODE_BYTES: usize = 64;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Discover,
    Check,
}
impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discover => "discover",
            Self::Check => "check",
        }
    }
}
/// Explicit optional contract features, independent of operation capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    NetworkV1,
}
/// Maximum negotiated features; currently only `network_v1` is defined.
pub const MAX_FEATURES: usize = 1;
pub(crate) fn validate_features(features: &[Feature]) -> Result<(), ProtocolError> {
    if features.len() > MAX_FEATURES
        || features
            .iter()
            .enumerate()
            .any(|(i, feature)| features[..i].contains(feature))
    {
        return Err(ProtocolError::Features);
    }
    Ok(())
}
/// Encode a pinned handshake naming the operation before credentials are delivered.
pub fn handshake_request(instance: &str, operation: Operation) -> Result<Vec<u8>, ProtocolError> {
    handshake_request_with_features(instance, operation, &[])
}
/// Request an exact optional feature set. Empty features preserve the original wire bytes.
pub fn handshake_request_with_features(
    instance: &str,
    operation: Operation,
    features: &[Feature],
) -> Result<Vec<u8>, ProtocolError> {
    if !crate::wire::valid_name(instance) {
        return Err(ProtocolError::Provider);
    }
    validate_features(features)?;
    let mut value = serde_json::json!({"protocol_version":PROTOCOL_VERSION,"id":"handshake","method":"handshake","instance":instance,"operation":operation.as_str()});
    if !features.is_empty() {
        value["features"] = serde_json::to_value(features).map_err(|_| ProtocolError::Schema)?;
    }
    let mut frame = serde_json::to_vec(&value).map_err(|_| ProtocolError::Schema)?;
    frame.push(b'\n');
    Ok(frame)
}
