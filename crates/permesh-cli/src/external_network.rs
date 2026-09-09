// SPDX-License-Identifier: MIT
//! Load pinned public CA material before any provider credential is resolved.
use crate::error::AppError;
use permesh_config::NetworkConfig;
use permesh_provider_sdk::network::{MAX_CA_BYTES, NetworkContext};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

#[derive(serde::Serialize)]
pub(super) struct NetworkReview<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    https_proxy: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    no_proxy: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    ca_bundle: Option<CaReview<'a>>,
}
#[derive(serde::Serialize)]
struct CaReview<'a> {
    path: &'a str,
    sha256: &'a str,
}
impl<'a> From<&'a NetworkConfig> for NetworkReview<'a> {
    fn from(config: &'a NetworkConfig) -> Self {
        Self {
            https_proxy: config.https_proxy.as_deref(),
            no_proxy: &config.no_proxy,
            ca_bundle: config.ca_bundle.as_ref().map(|ca| CaReview {
                path: &ca.path,
                sha256: &ca.sha256,
            }),
        }
    }
}

fn invalid() -> AppError {
    AppError::input(
        "External network CA bundle is missing, invalid, outside the workspace directory, or does not match its SHA-256 pin; review the file and network configuration before approving again",
    )
}

pub(super) fn load(config: &NetworkConfig, workspace: &Path) -> Result<NetworkContext, AppError> {
    config.validate()?;
    let ca_bundle_pem = config
        .ca_bundle
        .as_ref()
        .map(|ca| {
            let base = workspace
                .parent()
                .ok_or_else(invalid)?
                .canonicalize()
                .map_err(|_| invalid())?;
            let path = base.join(&ca.path).canonicalize().map_err(|_| invalid())?;
            if !path.starts_with(&base) || !path.is_file() {
                return Err(invalid());
            }
            let file = File::open(path).map_err(|_| invalid())?;
            if !file.metadata().map_err(|_| invalid())?.is_file() {
                return Err(invalid());
            }
            let mut bytes = Vec::new();
            file.take((MAX_CA_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| invalid())?;
            if bytes.len() > MAX_CA_BYTES
                || Sha256::digest(&bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    != ca.sha256
            {
                return Err(invalid());
            }
            String::from_utf8(bytes).map_err(|_| invalid())
        })
        .transpose()?;
    let context = NetworkContext {
        https_proxy: config.https_proxy.clone(),
        no_proxy: config.no_proxy.clone(),
        ca_bundle_pem,
    };
    context.validate().map_err(|_| invalid())?;
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_ca_rejects_malformed_oversized_and_missing_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let temporary = tempfile::tempdir()?;
        let workspace = temporary.path().join("permesh.yaml");
        let path = temporary.path().join("ca.pem");
        let mut config = NetworkConfig {
            https_proxy: None,
            no_proxy: vec![],
            ca_bundle: Some(permesh_config::CaBundle {
                path: "ca.pem".into(),
                sha256: "a".repeat(64),
            }),
        };
        assert!(load(&config, &workspace).is_err());
        for bytes in [
            b"-----BEGIN PRIVATE KEY-----\nYWJj\n-----END PRIVATE KEY-----\n".to_vec(),
            b"not a public certificate".to_vec(),
            vec![b'A'; MAX_CA_BYTES + 1],
        ] {
            std::fs::write(&path, &bytes)?;
            config.ca_bundle.as_mut().ok_or("ca")?.sha256 = Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let error = load(&config, &workspace)
                .err()
                .ok_or("accepted invalid CA")?;
            assert!(!error.message.contains("PRIVATE KEY"));
            assert!(!error.message.contains("not a public certificate"));
        }
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn relative_ca_symlink_cannot_escape_workspace_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        let workspace = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        let pem = "-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n";
        std::fs::write(outside.path().join("ca.pem"), pem)?;
        std::os::unix::fs::symlink(
            outside.path().join("ca.pem"),
            workspace.path().join("ca.pem"),
        )?;
        let config = NetworkConfig {
            https_proxy: None,
            no_proxy: vec![],
            ca_bundle: Some(permesh_config::CaBundle {
                path: "ca.pem".into(),
                sha256: Sha256::digest(pem.as_bytes())
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            }),
        };
        assert!(load(&config, &workspace.path().join("permesh.yaml")).is_err());
        Ok(())
    }
}
