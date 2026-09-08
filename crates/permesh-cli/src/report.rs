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
                schema_version: 1,
                command: command.into(),
                complete: true,
                started_at: timestamp.clone(),
                completed_at: timestamp,
                providers: vec![],
                result,
            },
            code: 0,
        })
    }
}
