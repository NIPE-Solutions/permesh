// SPDX-License-Identifier: MIT
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: u8,
    pub message: String,
    #[serde(skip)]
    pub diagnostic: Option<crate::provider_diagnostics::Code>,
}
impl AppError {
    pub fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            diagnostic: None,
        }
    }
    pub(crate) fn diagnostic(mut self, code: crate::provider_diagnostics::Code) -> Self {
        self.diagnostic = Some(code);
        self
    }
    pub fn input(message: impl Into<String>) -> Self {
        Self::new(2, message)
    }
}
impl From<permesh_config::Error> for AppError {
    fn from(error: permesh_config::Error) -> Self {
        Self::input(format!(
            "{error}. Check permesh.yaml or pass --config FILE."
        ))
    }
}
impl From<permesh_core::DomainError> for AppError {
    fn from(error: permesh_core::DomainError) -> Self {
        let code = match error {
            permesh_core::DomainError::NotFound => 1,
            permesh_core::DomainError::Ambiguous => 2,
            _ => 3,
        };
        Self::new(code, error.to_string())
    }
}

impl From<permesh_provider_sdk::ProviderError> for AppError {
    fn from(error: permesh_provider_sdk::ProviderError) -> Self {
        Self::new(3, error.message)
    }
}
