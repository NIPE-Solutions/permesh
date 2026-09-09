// SPDX-License-Identifier: MIT
use crate::{
    args::AuthCommand,
    collection,
    error::AppError,
    report::{Outcome, ProviderStatus},
};
use permesh_config::{Config, ProviderKind};
use permesh_secrets::{Secret, SecretRef, SecretResolver};
use std::io::{self, IsTerminal, Read};
const MAX_TOKEN_BYTES: u64 = 16_384;
pub async fn run(
    config: &Config,
    command: &AuthCommand,
    json: bool,
    blocking: &crate::blocking::BlockingPool,
) -> Result<Outcome, AppError> {
    let config = config.clone();
    let command = command.clone();
    blocking
        .run(move || run_sync(&config, &command, json))
        .await?
}
fn run_sync(config: &Config, command: &AuthCommand, json: bool) -> Result<Outcome, AppError> {
    match command {
        AuthCommand::Status { id } => {
            let mut o = Outcome::new(
                "auth_status",
                serde_json::json!({"message":"Local credential availability; provider authentication is checked by doctor."}),
            )?;
            for p in collection::selected(config, id.as_deref())? {
                if matches!(p.kind, ProviderKind::Github | ProviderKind::Google) {
                    o.report.providers.push(ProviderStatus {
                        id: p.id.clone(),
                        kind: collection::kind(&p).into(),
                        state: "failed".into(),
                        message: collection::legacy_message(&p),
                        limitations: vec![],
                    });
                    continue;
                }
                if p.external
                    .as_ref()
                    .is_some_and(|external| external.aws_profile.is_some())
                {
                    o.report.providers.push(ProviderStatus { id: p.id.clone(), kind: collection::kind(&p).into(), state: "configured".into(), message: "Explicit temporary AWS profile configured; file and session validity are unverified until an approved provider operation. No credential file was read.".into(), limitations: vec![] });
                    continue;
                }
                if p.external.as_ref().is_some_and(|external| {
                    external.credentials.values().any(|value| {
                        matches!(SecretRef::parse(value), Ok(SecretRef::Remote { .. }))
                    })
                }) {
                    let local_available = p.external.as_ref().is_some_and(|external| {
                        external
                            .credentials
                            .values()
                            .all(|value| match SecretRef::parse(value) {
                                Ok(SecretRef::Remote { .. }) => true,
                                Ok(local) => SecretResolver.resolve(&local).is_ok(),
                                Err(_) => false,
                            })
                    });
                    o.report.providers.push(ProviderStatus {
                        id: p.id.clone(), kind: collection::kind(&p).into(), state: if local_available {"configured"} else {"failed"}.into(),
                        message: if local_available {"Remote credential resolver configured; availability is unverified until an approved provider operation. No remote store or bootstrap credential was accessed."} else {"A direct local credential is unavailable. Remote resolver availability remains unverified; no remote store or bootstrap credential was accessed."}.into(), limitations: vec![],
                    });
                    continue;
                }
                let available = if p.kind == ProviderKind::Demo {
                    true
                } else if let Some(external) = &p.external {
                    external.credentials.values().all(|value| {
                        SecretRef::parse(value).is_ok_and(|r| SecretResolver.resolve(&r).is_ok())
                    })
                } else {
                    p.auth
                        .as_ref()
                        .and_then(|a| SecretRef::parse(&a.token).ok())
                        .is_some_and(|r| SecretResolver.resolve(&r).is_ok())
                };
                o.report.providers.push(ProviderStatus {
                    id: p.id.clone(),
                    kind: collection::kind(&p).into(),
                    state: if available { "connected" } else { "failed" }.into(),
                    message: if available {
                        "Credential available (or not required)"
                    } else {
                        "Credential unavailable; set environment reference or run auth login"
                    }
                    .into(),
                    limitations: vec![],
                });
            }
            o.report.providers.sort_by(|a, b| a.id.cmp(&b.id));
            o.report.complete = o
                .report
                .providers
                .iter()
                .all(|p| matches!(p.state.as_str(), "connected" | "configured"));
            if !o.report.complete {
                o.code = if o.report.providers.iter().all(|p| p.state == "failed") {
                    3
                } else {
                    4
                };
            }
            Ok(o)
        }
        AuthCommand::Login {
            id,
            token_stdin,
            credential,
            ..
        } => {
            let reference = keychain_ref(config, id, credential.as_deref())?;
            let value = if *token_stdin {
                let mut bytes =
                    zeroize::Zeroizing::new(Vec::with_capacity(MAX_TOKEN_BYTES as usize + 1));
                io::stdin()
                    .take(MAX_TOKEN_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| AppError::input("Cannot read token from stdin"))?;
                if bytes.len() as u64 > MAX_TOKEN_BYTES {
                    return Err(AppError::input("Token exceeds 16 KiB limit"));
                }
                let text = std::str::from_utf8(&bytes)
                    .map_err(|_| AppError::input("Token must be UTF-8 text"))?;
                zeroize::Zeroizing::new(text.to_string())
            } else {
                if json || !io::stdin().is_terminal() {
                    return Err(AppError::input(
                        "Use auth login INSTANCE --token-stdin for noninteractive login, or configure an env:// reference.",
                    ));
                }
                zeroize::Zeroizing::new(
                    rpassword::prompt_password("Provider token (hidden): ")
                        .map_err(|_| AppError::input("Cannot read token from terminal"))?,
                )
            };
            let secret = Secret::new(value.trim_end_matches(['\r', '\n']).to_string());
            if secret.expose().is_empty() || secret.expose().chars().any(char::is_control) {
                return Err(AppError::input(
                    "Token must be nonempty text without control characters",
                ));
            }
            permesh_secrets::store(&reference,&secret).map_err(|_|AppError::new(3,"Cannot store token in native keychain. Check OS credential storage availability, or use an env:// reference."))?;
            Outcome::new(
                "auth_login",
                serde_json::json!({"message":"Stored local keychain credential. Run permesh doctor to verify provider authentication and visibility."}),
            )
        }
        AuthCommand::Logout { id, credential } => {
            permesh_secrets::delete(&keychain_ref(config,id,credential.as_deref())?).map_err(|_|AppError::new(3,"Cannot delete keychain credential. Check that it exists and native storage is available."))?;
            Outcome::new(
                "auth_logout",
                serde_json::json!({"message":"Deleted local keychain credential. This does not revoke the token at its provider."}),
            )
        }
    }
}
fn keychain_ref(
    config: &Config,
    id: &str,
    credential: Option<&str>,
) -> Result<SecretRef, AppError> {
    let provider = config
        .providers
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::input("Unknown provider instance; run permesh provider list"))?;
    if matches!(provider.kind, ProviderKind::Github | ProviderKind::Google) {
        return Err(AppError::input(collection::legacy_message(provider)));
    }
    if provider
        .external
        .as_ref()
        .is_some_and(|external| external.aws_profile.is_some())
    {
        return Err(AppError::input(
            "This instance reads an explicit temporary AWS profile. Refresh or remove the selected profile using your existing AWS authentication tooling; auth login/logout does not modify shared AWS files.",
        ));
    }
    let value = if let Some(external) = &provider.external {
        let name = credential
            .or_else(|| {
                if external.credentials.contains_key("token") {
                    Some("token")
                } else if external.credentials.len() == 1 {
                    external.credentials.keys().next().map(String::as_str)
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                AppError::input("Select a configured credential with --credential NAME")
            })?;
        external
            .credentials
            .get(name)
            .ok_or_else(|| AppError::input("Unknown credential slot"))?
    } else {
        if credential.is_some() {
            return Err(AppError::input(
                "--credential applies to external providers",
            ));
        }
        &provider
            .auth
            .as_ref()
            .ok_or_else(|| AppError::input("This provider does not require authentication"))?
            .token
    };
    let reference =
        SecretRef::parse(value).map_err(|_| AppError::input("Invalid secret reference"))?;
    if matches!(reference, SecretRef::Remote { .. }) {
        return Err(AppError::input(
            "This credential slot uses a read-only remote resolver. Set its configured bootstrap environment or instance keychain entry; auth login/logout never writes remote stores.",
        ));
    }
    if matches!(reference, SecretRef::Env(_)) {
        return Err(AppError::input(
            "This instance uses an environment reference. Set or unset that variable in your shell; auth login/logout only manage keychain references.",
        ));
    }
    Ok(reference)
}

#[cfg(test)]
mod remote_tests {
    use super::*;
    #[test]
    fn remote_status_is_configured_unverified_and_login_logout_never_touch_bootstrap()
    -> Result<(), Box<dyn std::error::Error>> {
        let config=Config::from_bytes(serde_json::json!({"version":1,"organization":{"name":"test"},"providers":[{"id":"instance","type":"external","external":{"provider":"fixture","sha256":"a".repeat(64),"credentials":{"token":"vault://named"},"credential_resolvers":{"named":{"version":1,"type":"vault_kv2","origin":"https://invalid.invalid","mount":"secret","path":"one","field":"token","bootstrap":"env://PERMESH_ABSENT_BOOTSTRAP_937812"}}}}]}).to_string().as_bytes())?;
        let status = run_sync(
            &config,
            &AuthCommand::Status {
                id: Some("instance".into()),
            },
            true,
        )
        .map_err(|e| e.message)?;
        assert_eq!(status.code, 0);
        assert!(status.report.complete);
        assert_eq!(status.report.providers[0].state, "configured");
        assert!(status.report.providers[0].message.contains("unverified"));
        let mut mixed = config.clone();
        mixed.providers[0]
            .external
            .as_mut()
            .ok_or("external")?
            .credentials
            .insert("other".into(), "env://PERMESH_ABSENT_DIRECT_938172".into());
        let mixed_status = run_sync(
            &mixed,
            &AuthCommand::Status {
                id: Some("instance".into()),
            },
            true,
        )
        .map_err(|e| e.message)?;
        assert_eq!(mixed_status.code, 3);
        assert!(!mixed_status.report.complete);
        assert!(
            mixed_status.report.providers[0]
                .message
                .contains("direct local credential")
        );
        let error = keychain_ref(&config, "instance", None)
            .err()
            .ok_or("remote slot must reject writes")?;
        assert_eq!(error.code, 2);
        assert!(error.message.contains("read-only remote resolver"));
        Ok(())
    }
}
