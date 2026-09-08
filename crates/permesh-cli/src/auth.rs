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
                if p.kind == ProviderKind::Github {
                    o.report.providers.push(ProviderStatus {
                        id: p.id.clone(),
                        kind: "github".into(),
                        state: "failed".into(),
                        message: collection::legacy_github_message(&p.id),
                        limitations: vec![],
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
            o.report.complete = o.report.providers.iter().all(|p| p.state == "connected");
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
    if provider.kind == ProviderKind::Github {
        return Err(AppError::input(collection::legacy_github_message(
            &provider.id,
        )));
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
    if matches!(reference, SecretRef::Env(_)) {
        return Err(AppError::input(
            "This instance uses an environment reference. Set or unset that variable in your shell; auth login/logout only manage keychain references.",
        ));
    }
    Ok(reference)
}
