// SPDX-License-Identifier: MIT
use crate::records::{Capability, Record};
use crate::{PROTOCOL_VERSION, ProtocolError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub(crate) struct Envelope {
    pub protocol: u32,
    pub id: String,
    #[serde(flatten)]
    pub event: Event,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub(crate) enum Event {
    Handshake {
        provider: String,
        capabilities: Vec<Capability>,
        draft: bool,
    },
    Record {
        #[serde(flatten)]
        record: Record,
    },
    Complete {
        count: usize,
        complete: bool,
        limitations: Vec<Limitation>,
    },
    Error {
        code: ErrorCode,
    },
    Health {
        status: HealthStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limitations: Option<Vec<Limitation>>,
    },
    Setup {
        spec: permesh_provider_sdk::setup::SetupSpec,
    },
    Auth {
        spec: permesh_provider_sdk::browser_auth::BrowserAuthSpec,
    },
    Cancelled,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HealthStatus {
    Ok,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorCode {
    ProtocolError,
    UnsupportedMethod,
    HandshakeRequired,
    Authentication,
    PermissionDenied,
    RateLimited,
    Unavailable,
    Internal,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Limitation {
    VisibilityLimited,
    PermissionDenied,
    RateLimited,
    PageLimit,
    Unknown,
}
impl Limitation {
    pub fn message(&self) -> &'static str {
        match self {
            Self::VisibilityLimited => "External provider reports limited visibility",
            Self::PermissionDenied => "External provider reports a permission denial",
            Self::RateLimited => "External provider reports a rate limit",
            Self::PageLimit => "External provider reached a page limit",
            Self::Unknown => "External provider reports an unspecified discovery limitation",
        }
    }
}
impl Envelope {
    pub fn parse(value: Value, version: u32) -> Result<Self, ProtocolError> {
        let envelope: Self =
            serde_json::from_value(value.clone()).map_err(|_| ProtocolError::Schema)?;
        // Flattened event envelopes need a lossless structural round trip to
        // reject unknown fields at every nesting level, including SDK specs.
        // Duplicate keys have already been rejected by the bounded JSON decoder.
        if serde_json::to_value(&envelope).map_err(|_| ProtocolError::Schema)? != value {
            return Err(ProtocolError::Schema);
        }
        if envelope.protocol != version {
            return Err(ProtocolError::Version);
        }
        Ok(envelope)
    }
}
pub(crate) fn valid_name(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
/// Encode the first request of a single-operation draft protocol process.
pub fn handshake_request(instance: &str) -> Result<Vec<u8>, ProtocolError> {
    handshake_request_versioned(instance, PROTOCOL_VERSION)
}
/// Encode a handshake pinned to supported draft version 1, 2, 3, or 4. Never downgrade.
pub fn handshake_request_versioned(instance: &str, version: u32) -> Result<Vec<u8>, ProtocolError> {
    validate_version(version)?;
    if !valid_name(instance) {
        return Err(ProtocolError::Provider);
    }
    let mut frame = serde_json::to_vec(&serde_json::json!({
        "protocol": version, "id":"handshake", "method":"handshake", "instance":instance
    }))
    .map_err(|_| ProtocolError::Schema)?;
    frame.push(b'\n');
    Ok(frame)
}

pub(crate) fn validate_version(version: u32) -> Result<(), ProtocolError> {
    if matches!(version, 1..=4) {
        Ok(())
    } else {
        Err(ProtocolError::Version)
    }
}
