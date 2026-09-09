// SPDX-License-Identifier: MIT
//! Pinned launch entrypoints for operations without an Invocation.
use super::*;

/// Discover with the verified registration digest rechecked immediately before launch.
/// Path-based execution still cannot exclude hostile same-user filesystem races.
pub async fn discover_pinned(
    executable: &Path,
    provider: &str,
    instance: &str,
    capabilities: &[Capability],
    expected_sha256: &str,
    cancellation: impl Future<Output = ()>,
) -> Result<Snapshot, ExternalError> {
    let decoder = DiscoveryDecoder::new(provider, instance, Some(capabilities))
        .map_err(|_| ExternalError::Input)?;
    let handshake = handshake_request(instance).map_err(|_| ExternalError::Input)?;
    supervise(
        executable,
        Exchange {
            decoder,
            handshake,
            invocation: None,
            method: "discover",
            contract: Contract::Legacy(1),
            expected_pin: Some(expected_sha256),
        },
        cancellation,
        Deadlines::default(),
    )
    .await
}
/// Describe setup after rechecking the verified registration digest before launch.
pub async fn describe_pinned(
    executable: &Path,
    provider: &str,
    instance: &str,
    capabilities: &[Capability],
    expected_sha256: &str,
    cancellation: impl Future<Output = ()>,
) -> Result<SetupSpec, ExternalError> {
    let decoder = SetupDecoder::new(provider, instance, Some(capabilities))
        .map_err(|_| ExternalError::Input)?;
    let handshake = handshake_request_versioned(instance, 3).map_err(|_| ExternalError::Input)?;
    supervise(
        executable,
        Exchange {
            decoder,
            handshake,
            invocation: None,
            method: "describe",
            contract: Contract::Legacy(3),
            expected_pin: Some(expected_sha256),
        },
        cancellation,
        Deadlines::default(),
    )
    .await
}
/// Describe browser authentication after rechecking the registration digest before launch.
pub async fn describe_auth_pinned(
    executable: &Path,
    provider: &str,
    instance: &str,
    capabilities: &[Capability],
    expected_sha256: &str,
    cancellation: impl Future<Output = ()>,
) -> Result<permesh_provider_sdk::browser_auth::BrowserAuthSpec, ExternalError> {
    let decoder = BrowserAuthDecoder::new(provider, instance, Some(capabilities))
        .map_err(|_| ExternalError::Input)?;
    let handshake = handshake_request_versioned(instance, 4).map_err(|_| ExternalError::Input)?;
    supervise(
        executable,
        Exchange {
            decoder,
            handshake,
            invocation: None,
            method: "describe_auth",
            contract: Contract::Legacy(4),
            expected_pin: Some(expected_sha256),
        },
        cancellation,
        Deadlines::default(),
    )
    .await
}
