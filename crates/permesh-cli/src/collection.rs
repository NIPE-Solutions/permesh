// SPDX-License-Identifier: MIT
use crate::{
    error::AppError,
    report::{Outcome, ProviderStatus, now},
};
use permesh_config::{Config, ProviderConfig, ProviderKind};
use permesh_core::Snapshot;
use permesh_provider_sdk::{Metadata, Provider, ProviderError};
use permesh_secrets::{SecretRef, SecretResolver};
use std::{collections::VecDeque, sync::Arc};
const MAX_CONCURRENT_PROVIDERS: usize = 4;
pub fn kind(provider: &ProviderConfig) -> &'static str {
    match provider.kind {
        ProviderKind::Demo => "demo",
        ProviderKind::Github => "github",
        ProviderKind::Google => "google",
        ProviderKind::External => "external",
    }
}
pub fn metadata(provider: &ProviderConfig) -> Result<Metadata, AppError> {
    Ok(match provider.kind {
        ProviderKind::Demo => permesh_provider_demo::provider_metadata(),
        ProviderKind::Github => permesh_provider_github::provider_metadata(),
        ProviderKind::Google => permesh_provider_google::provider_metadata(),
        ProviderKind::External => return crate::external_workspace::metadata(provider),
    })
}
pub fn build(provider: &ProviderConfig) -> Result<Arc<dyn Provider>, ProviderError> {
    match provider.kind {
        ProviderKind::External => Err(ProviderError::new(
            "external",
            "External providers require workspace approval",
        )),
        ProviderKind::Demo => Ok(Arc::new(permesh_provider_demo::DemoProvider::new(
            &provider.id,
        ))),
        ProviderKind::Github | ProviderKind::Google => {
            let reference = provider
                .auth
                .as_ref()
                .ok_or_else(|| ProviderError::new("auth", "Authentication reference missing"))?;
            let reference = SecretRef::parse(&reference.token)
                .map_err(|_| ProviderError::new("auth", "Invalid secret reference"))?;
            let secret=SecretResolver.resolve(&reference).map_err(|_|ProviderError::new("auth","Authentication unavailable. Set the configured environment variable or run permesh auth login for this instance."))?;
            if provider.kind == ProviderKind::Google {
                let customer = provider.customer_id.clone().ok_or_else(|| {
                    ProviderError::new("configuration", "Google customer_id missing")
                })?;
                Ok(Arc::new(permesh_provider_google::GoogleProvider::new(
                    provider.id.clone(),
                    customer,
                    secret,
                )?))
            } else {
                Ok(Arc::new(permesh_provider_github::GithubProvider::new(
                    provider.id.clone(),
                    provider.organizations.clone(),
                    secret,
                )?))
            }
        }
    }
}
pub fn selected(config: &Config, id: Option<&str>) -> Result<Vec<ProviderConfig>, AppError> {
    let selected: Vec<_> = config
        .providers
        .iter()
        .filter(|p| id.is_none_or(|id| p.id == id))
        .cloned()
        .collect();
    if id.is_some() && selected.is_empty() {
        return Err(AppError::input(
            "Unknown provider instance; run permesh provider list",
        ));
    }
    Ok(selected)
}
pub async fn collect(
    blocking: &crate::blocking::BlockingPool,
    config: &Config,
    path: &std::path::Path,
    cancellation: &crate::cancellation::Cancellation,
    providers: Vec<ProviderConfig>,
    discovery: bool,
    command: &str,
) -> Result<(Outcome, Vec<Snapshot>), AppError> {
    let mut outcome = Outcome::new(command, serde_json::json!({}))?;
    let mut pending: VecDeque<_> = providers.into();
    let mut tasks = tokio::task::JoinSet::new();
    let mut snapshots = vec![];
    let config_owned = Arc::new(config.clone());
    let mut internal_failure = false;
    while !pending.is_empty() || !tasks.is_empty() {
        if cancellation.is_cancelled() {
            pending.clear();
        }
        while tasks.len() < MAX_CONCURRENT_PROVIDERS && !cancellation.is_cancelled() {
            let Some(provider) = pending.pop_front() else {
                break;
            };
            let blocking = blocking.clone();
            let config = config_owned.clone();
            let path = path.to_owned();
            let cancellation = cancellation.clone();
            tasks.spawn(async move {
                let id = provider.id.clone();
                let kind = kind(&provider).to_string();
                let result = crate::provider_operation::run(
                    &blocking,
                    &config,
                    &path,
                    provider,
                    discovery,
                    &cancellation,
                )
                .await;
                (id, kind, result)
            });
        }
        let Some(joined) = tasks.join_next().await else {
            break;
        };
        let (id, kind, result) = match joined {
            Ok(result) => result,
            Err(_) => {
                internal_failure = true;
                cancellation.cancel();
                continue;
            }
        };
        if result.as_ref().is_err_and(|error| error.code == 5) {
            internal_failure = true;
        }
        match result {
            Ok((message, limitations, snapshot)) => {
                let complete = snapshot.as_ref().is_none_or(|s| s.complete);
                outcome.report.providers.push(ProviderStatus {
                    id,
                    kind,
                    state: if complete { "connected" } else { "partial" }.into(),
                    message,
                    limitations,
                });
                if let Some(mut snapshot) = snapshot {
                    if !config
                        .identity
                        .sources
                        .iter()
                        .any(|s| s.provider == snapshot.provider && s.authoritative)
                    {
                        snapshot.identities.clear();
                    }
                    snapshots.push(snapshot);
                }
            }
            Err(error) => outcome.report.providers.push(ProviderStatus {
                id,
                kind,
                state: "failed".into(),
                message: error.message,
                limitations: vec![],
            }),
        }
    }
    if internal_failure {
        return Err(AppError::new(
            5,
            "Provider task or process cleanup failed; no diagnostic payload was retained",
        ));
    }
    if cancellation.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    outcome.report.providers.sort_by(|a, b| a.id.cmp(&b.id));
    snapshots.sort_by(|a, b| a.provider.cmp(&b.provider));
    let failures = outcome
        .report
        .providers
        .iter()
        .filter(|p| p.state != "connected")
        .count();
    outcome.report.complete = failures == 0;
    outcome.code = if failures == 0 {
        0
    } else if outcome.report.providers.iter().all(|p| p.state == "failed") {
        3
    } else {
        4
    };
    outcome.report.completed_at = now()?;
    Ok((outcome, snapshots))
}
