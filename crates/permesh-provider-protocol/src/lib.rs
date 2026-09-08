// SPDX-License-Identifier: MIT
//! Offline validation for draft external discovery exchanges. Does not execute programs.
mod framing;
mod session;
mod wire;
pub use session::{
    BrowserAuthDecoder, DiscoveryDecoder, HealthDecoder, Progress, SetupDecoder, validate_discovery,
};
pub use wire::{handshake_request, handshake_request_versioned};

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const MAX_TRANSCRIPT_BYTES: usize = 64 * 1_048_576;
pub const MAX_RECORDS: usize = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error("cannot read provider protocol input")]
    Io,
    #[error("provider protocol frame exceeds the byte limit")]
    FrameLimit,
    #[error("provider protocol exchange exceeds the byte limit")]
    TranscriptLimit,
    #[error("provider protocol frame is not terminated by LF")]
    Truncated,
    #[error("provider protocol contains invalid JSON, duplicate keys, or excessive nesting")]
    Json,
    #[error("provider protocol fields or record schema are invalid")]
    Schema,
    #[error("provider protocol version is unsupported")]
    Version,
    #[error("provider protocol request ID or event order is invalid")]
    Sequence,
    #[error("provider protocol identity does not match the expected provider")]
    Provider,
    #[error("provider protocol capability declaration or record kind is invalid")]
    Capability,
    #[error("provider protocol record limit exceeded")]
    RecordLimit,
    #[error("provider protocol completion count is invalid")]
    Count,
    #[error("provider protocol ended without a completion event")]
    Incomplete,
    #[error("external provider reported failure; no snapshot accepted")]
    ProviderFailed,
    #[error("provider protocol snapshot failed domain validation")]
    Snapshot,
}
