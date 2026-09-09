// SPDX-License-Identifier: MIT
use super::PROTOCOL_VERSION;
use super::records::{Capability, Record};
use crate::ProtocolError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub(crate) struct Envelope {
    pub protocol_version: u32,
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
        operations: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        features: Option<Vec<super::Feature>>,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_code: Option<String>,
    },
    Health {
        status: HealthStatus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limitations: Option<Vec<Limitation>>,
    },
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
    pub fn parse(value: Value) -> Result<Self, ProtocolError> {
        let envelope: Self =
            serde_json::from_value(value.clone()).map_err(|_| ProtocolError::Schema)?;
        // Flattened event envelopes need a lossless structural round trip to
        // reject unknown fields at every nesting level, including nested record keys.
        // Duplicate keys have already been rejected by the bounded JSON decoder.
        if serde_json::to_value(&envelope).map_err(|_| ProtocolError::Schema)? != value {
            return Err(ProtocolError::Schema);
        }
        if envelope.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolError::Version);
        }
        if let Event::Error {
            provider_code: Some(code),
            ..
        } = &envelope.event
            && (!(1..=super::MAX_PROVIDER_CODE_BYTES).contains(&code.len())
                || !code.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
                }))
        {
            return Err(ProtocolError::Schema);
        }
        Ok(envelope)
    }
}
