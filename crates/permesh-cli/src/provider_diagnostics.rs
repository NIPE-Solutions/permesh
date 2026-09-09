// SPDX-License-Identifier: MIT
//! Curated CLI diagnostic identifiers; never derived from provider error text.
use permesh_provider_external::ExternalError;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Code {
    CheckOk,
    VisibilityLimited,
    InvalidConfiguration,
    MigrationRequired,
    TargetPinUnavailable,
    BinaryUntrustedOrChanged,
    StorageUnavailable,
    ApprovalMissingOrStale,
    NetworkContextInvalid,
    CredentialUnavailable,
    ExecutableChanged,
    SpawnFailed,
    NetworkFeatureUnsupported,
    ProtocolInvalid,
    ProviderFailed,
    DeadlineExceeded,
    DiagnosticLimitExceeded,
    CleanupFailed,
    Cancelled,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Stage {
    Configuration,
    Pin,
    Trust,
    Approval,
    Network,
    Credentials,
    Launch,
    Handshake,
    Health,
    Operation,
    Cleanup,
}
#[derive(Serialize)]
pub(crate) struct Row {
    pub instance: String,
    stage: Stage,
    code: Code,
    message: &'static str,
    next: String,
}
impl Code {
    pub(crate) fn row(self, instance: &str) -> Row {
        let (stage, message, next) = match self {
            Self::CheckOk => (Stage::Health, "Provider health check succeeded; discovery visibility was not enumerated.", "Use a query to inspect observed access.".into()),
            Self::VisibilityLimited => (Stage::Health, "Provider health check succeeded with visibility limitations.", "Review the reported limitations before relying on discovery coverage.".into()),
            Self::InvalidConfiguration => (Stage::Configuration, "Provider configuration is invalid.", "Correct permesh.yaml, then run permesh doctor --details.".into()),
            Self::MigrationRequired => (Stage::Configuration, "This legacy provider instance requires explicit migration.", format!("Install and trust its external provider, then run permesh provider migrate {instance} --sha256 DIGEST --discovery-protocol negotiated-v1 for a negotiated-v1 binary.")),
            Self::TargetPinUnavailable => (Stage::Pin, "No reviewed provider digest is available for this native target.", "Add the reviewed native-target digest to external.sha256_by_target in permesh.yaml.".into()),
            Self::BinaryUntrustedOrChanged => (Stage::Trust, "The pinned binary registration is missing, invalid or changed.", format!("Run permesh provider external list, then inspect and explicitly trust the pinned binary; run permesh provider external review {instance}.")),
            Self::StorageUnavailable => (Stage::Trust, "Protected local provider storage is unavailable.", "Check provider storage ownership, permissions and concurrent operations; retry permesh doctor --details.".into()),
            Self::ApprovalMissingOrStale => (Stage::Approval, "This provider context has no current workspace approval.", format!("Run permesh provider external review {instance}, then approve its reviewed fingerprint with permesh provider external approve {instance} --fingerprint REVIEWED_FINGERPRINT --accept-risk.")),
            Self::NetworkContextInvalid => (Stage::Network, "Explicit proxy or pinned CA configuration failed local validation.", format!("Correct the network settings or CA file and SHA-256 pin; run permesh provider external review {instance} before approving again.")),
            Self::CredentialUnavailable => (Stage::Credentials, "A configured credential reference could not be resolved.", format!("Set the configured environment variable, or use permesh auth login {instance} --credential SLOT for its configured keychain slot.")),
            Self::ExecutableChanged => (Stage::Launch, "The executable failed its final launch integrity check.", format!("Reinspect and explicitly trust the intended binary, then run permesh provider external review {instance}.")),
            Self::SpawnFailed => (Stage::Launch, "The native provider process could not be started.", "Check native target compatibility and executable permissions; reinstall the exact pinned package if needed.".into()),
            Self::NetworkFeatureUnsupported => (Stage::Handshake, "The provider did not accept the required network contract; credentials were withheld.", "Use a provider supporting network_v1 or explicitly revise the network configuration and review approval; no automatic downgrade is available.".into()),
            Self::ProtocolInvalid => (Stage::Operation, "The provider exchange failed protocol validation.", "Check the configured discovery protocol and provider compatibility; inspect an updated provider before changing its pin.".into()),
            Self::ProviderFailed => (Stage::Operation, "The provider operation failed; no remote error payload is exposed.", "Check service reachability and provider configuration, then rerun permesh doctor --details.".into()),
            Self::DeadlineExceeded => (Stage::Operation, "Provider preparation or execution exceeded its deadline.", "Check credential-store responsiveness and provider connectivity, then retry permesh doctor --details.".into()),
            Self::DiagnosticLimitExceeded => (Stage::Operation, "Provider diagnostic output exceeded the allowed bound.", "Use a compatible provider with bounded diagnostic output.".into()),
            Self::CleanupFailed => (Stage::Cleanup, "Native process cleanup failed.", "Investigate remaining provider processes before retrying.".into()),
            Self::Cancelled => (Stage::Operation, "The provider operation was cancelled.", "Retry the command when ready.".into()),
        };
        Row {
            instance: instance.into(),
            stage,
            code: self,
            message,
            next,
        }
    }
    pub(crate) fn host(error: &ExternalError) -> Self {
        match error {
            ExternalError::Input => Self::InvalidConfiguration,
            ExternalError::Trust => Self::ExecutableChanged,
            ExternalError::Storage => Self::StorageUnavailable,
            ExternalError::Spawn => Self::SpawnFailed,
            ExternalError::Timeout => Self::DeadlineExceeded,
            ExternalError::Cancelled => Self::Cancelled,
            ExternalError::Protocol => Self::ProtocolInvalid,
            ExternalError::NetworkNegotiation => Self::NetworkFeatureUnsupported,
            ExternalError::StderrLimit => Self::DiagnosticLimitExceeded,
            ExternalError::Exit => Self::ProviderFailed,
            ExternalError::Cleanup => Self::CleanupFailed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_host_failures_have_curated_codes_without_raw_payloads()
    -> Result<(), Box<dyn std::error::Error>> {
        for (error, expected) in [
            (ExternalError::Input, Code::InvalidConfiguration),
            (ExternalError::Trust, Code::ExecutableChanged),
            (ExternalError::Storage, Code::StorageUnavailable),
            (ExternalError::Spawn, Code::SpawnFailed),
            (ExternalError::Timeout, Code::DeadlineExceeded),
            (ExternalError::Cancelled, Code::Cancelled),
            (ExternalError::Protocol, Code::ProtocolInvalid),
            (
                ExternalError::NetworkNegotiation,
                Code::NetworkFeatureUnsupported,
            ),
            (ExternalError::StderrLimit, Code::DiagnosticLimitExceeded),
            (ExternalError::Exit, Code::ProviderFailed),
            (ExternalError::Cleanup, Code::CleanupFailed),
        ] {
            assert_eq!(Code::host(&error), expected);
        }
        let error =
            crate::error::AppError::new(3, "SENTINEL_PRIVATE").diagnostic(Code::ProtocolInvalid);
        let row = serde_json::to_string(&error.diagnostic.ok_or("diagnostic")?.row("instance"))?;
        assert!(!row.contains("SENTINEL_PRIVATE"));
        assert!(row.contains("protocol_invalid"));
        let serialized = serde_json::to_value(&error)?;
        assert!(serialized.get("diagnostic").is_none());
        assert_eq!(crate::external::failure(ExternalError::Cleanup).code, 5);
        assert_eq!(crate::external::failure(ExternalError::Cancelled).code, 130);
        Ok(())
    }
}
