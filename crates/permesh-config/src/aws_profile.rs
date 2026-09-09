// SPDX-License-Identifier: MIT
//! Explicit host-local temporary AWS credentials source; never an SDK fallback chain.
use crate::{ExternalConfig, Result, invalid};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwsProfile {
    pub version: u32,
    pub credentials_file: String,
    pub profile: String,
}
impl AwsProfile {
    pub(crate) fn validate(&self, external: &ExternalConfig) -> Result<()> {
        let text = |key| {
            external
                .configuration
                .get(key)
                .and_then(serde_json::Value::as_str)
        };
        let valid_role = text("caller_role").is_some_and(|v| {
            !v.is_empty()
                && v.len() <= 64
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"+=,.@_-".contains(&b))
        });
        let valid_account = text("account_id")
            .is_some_and(|v| v.len() == 12 && v.bytes().all(|b| b.is_ascii_digit()));
        let valid_region = text("region").is_some_and(|v| {
            !v.is_empty()
                && v.len() <= 32
                && v.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });
        if self.version != 1
            || self.credentials_file.len() > 4096
            || self.credentials_file.chars().any(char::is_control)
            || !std::path::Path::new(&self.credentials_file).is_absolute()
            || std::path::Path::new(&self.credentials_file)
                .components()
                .any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir | std::path::Component::CurDir
                    )
                })
            || self.profile.is_empty()
            || self.profile.len() > 128
            || !self
                .profile
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            || !external.credentials.is_empty()
            || !external.credential_resolvers.is_empty()
            || !valid_account
            || !valid_role
            || !valid_region
        {
            return Err(invalid(
                "external.aws_profile requires version 1, an explicit absolute credentials_file, a bounded named profile, no other credential sources, and explicit account_id, region and caller_role settings",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn external() -> ExternalConfig {
        serde_json::from_value(serde_json::json!({"provider":"aws","sha256":"a".repeat(64),"configuration":{"account_id":"123456789012","region":"eu-west-1","caller_role":"Reader"},"credentials":{}})).unwrap_or_else(|_| unreachable!("fixed fixture"))
    }
    #[test]
    fn explicit_profile_configuration_has_no_implicit_credential_precedence()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut config = external();
        let profile = AwsProfile {
            version: 1,
            credentials_file: std::env::temp_dir()
                .join("credentials")
                .to_string_lossy()
                .into_owned(),
            profile: "review".into(),
        };
        profile.validate(&config)?;
        for missing in ["account_id", "region", "caller_role"] {
            let mut invalid = config.clone();
            invalid.configuration.remove(missing);
            assert!(profile.validate(&invalid).is_err());
        }
        config
            .credentials
            .insert("token".into(), "env://TOKEN".into());
        assert!(profile.validate(&config).is_err());
        let mut relative = profile.clone();
        relative.credentials_file = "credentials".into();
        assert!(relative.validate(&external()).is_err());
        assert!(serde_json::from_value::<AwsProfile>(serde_json::json!({"version":1,"credentials_file":"/credentials","profile":"review","credential_process":"no"})).is_err());
        assert!(
            serde_json::to_value(external())?
                .get("aws_profile")
                .is_none()
        );
        Ok(())
    }
}
