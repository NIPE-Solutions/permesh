// SPDX-License-Identifier: MIT OR Apache-2.0
pub mod approvals;
pub mod catalog;
pub mod download;
pub mod host;
mod invocation;
pub mod packages;
pub mod trust;

#[derive(Debug, thiserror::Error)]
pub enum DistributionError {
    #[error(
        "invalid provider distribution input; check the provider name and exact stable version"
    )]
    Input,
    #[error("provider catalog failed validation; no package was installed")]
    Catalog,
    #[error("no compatible provider release was found for this version and platform")]
    Compatibility,
    #[error(
        "provider download failed or exceeded its deadline; check connectivity to GitHub and retry"
    )]
    Network,
    #[error(
        "provider package failed integrity validation; existing installations were not replaced"
    )]
    Integrity,
    #[error(
        "cannot access provider package storage; check its permissions and concurrent installations"
    )]
    Storage,
}

#[derive(Debug, thiserror::Error)]
pub enum ExternalError {
    #[error("invalid external provider input")]
    Input,
    #[error(
        "external provider trust state is missing, invalid or changed; inspect and explicitly trust the binary again"
    )]
    Trust,
    #[error("cannot access external provider storage")]
    Storage,
    #[error("external provider could not be started")]
    Spawn,
    #[error("external provider exceeded its deadline")]
    Timeout,
    #[error("external provider discovery cancelled")]
    Cancelled,
    #[error("external provider protocol failed validation")]
    Protocol,
    #[error("external provider diagnostic output exceeded its limit")]
    StderrLimit,
    #[error("external provider exited unsuccessfully")]
    Exit,
    #[error("external provider process cleanup failed")]
    Cleanup,
}
