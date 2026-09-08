// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{PROTOCOL_VERSION, ProtocolError};
use permesh_core::{Account, Grant, Group, Identity, Membership, Resource};
use permesh_provider_sdk::Capability;
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
    },
    Cancelled,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub(crate) enum Record {
    Identity(Identity),
    Account(Account),
    Resource(Resource),
    Group(Group),
    Membership(Membership),
    Grant(Grant),
}
impl Record {
    pub fn capability(&self) -> Capability {
        match self {
            Self::Identity(_) => Capability::Identities,
            Self::Account(_) => Capability::Accounts,
            Self::Resource(_) => Capability::Resources,
            Self::Group(_) => Capability::Groups,
            Self::Membership(_) => Capability::Memberships,
            Self::Grant(_) => Capability::Grants,
        }
    }
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
        // Domain deserializers tolerate extensions. The wire does not: a lossless
        // structural round trip rejects unknown fields at every nesting level.
        // Duplicate keys have already been rejected by the bounded JSON decoder.
        if serde_json::to_value(&envelope).map_err(|_| ProtocolError::Schema)? != value {
            return Err(ProtocolError::Schema);
        }
        if envelope.protocol != PROTOCOL_VERSION {
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
    if !valid_name(instance) {
        return Err(ProtocolError::Provider);
    }
    let mut frame = serde_json::to_vec(&serde_json::json!({
        "protocol": PROTOCOL_VERSION, "id":"handshake", "method":"handshake", "instance":instance
    }))
    .map_err(|_| ProtocolError::Schema)?;
    frame.push(b'\n');
    Ok(frame)
}
