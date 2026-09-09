// SPDX-License-Identifier: MIT
use crate::error::AppError;
use serde::Serialize;
#[derive(Serialize)]
pub struct ProviderStatus {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub message: String,
    pub limitations: Vec<String>,
}
#[derive(Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub command: String,
    pub complete: bool,
    pub started_at: String,
    pub completed_at: String,
    pub providers: Vec<ProviderStatus>,
    pub result: serde_json::Value,
}
pub struct Outcome {
    pub report: Report,
    pub code: u8,
    pub(crate) diagnostics: Vec<crate::provider_diagnostics::Row>,
}
pub fn now() -> Result<String, AppError> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| AppError::new(5, "Cannot format observation timestamp"))
}
impl Outcome {
    pub fn new(command: &str, result: serde_json::Value) -> Result<Self, AppError> {
        let timestamp = now()?;
        Ok(Self {
            report: Report {
                schema_version: schema_version(command),
                command: command.into(),
                complete: true,
                started_at: timestamp.clone(),
                completed_at: timestamp,
                providers: vec![],
                result,
            },
            code: 0,
            diagnostics: vec![],
        })
    }
}

/// Access reports migrated together; control reports retain their independent contract.
pub(crate) fn schema_version(command: &str) -> u32 {
    match command {
        "user" | "admins" | "orphaned" | "external_discover" => 2,
        _ => 1,
    }
}
pub(crate) fn command_schema_version(command: &crate::args::Command) -> u32 {
    use crate::args::{Command, ProviderCommand};
    match command {
        Command::User { .. }
        | Command::Admins
        | Command::Orphaned
        | Command::Provider {
            command:
                ProviderCommand::External {
                    command: crate::external::ExternalCommand::Discover { .. },
                },
        } => 2,
        _ => 1,
    }
}
