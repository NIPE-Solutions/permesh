// SPDX-License-Identifier: MIT OR Apache-2.0
pub mod host;
pub mod trust;

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
