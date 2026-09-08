// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{
    error::AppError,
    report::{Outcome, ProviderStatus, now},
};
use permesh_config::{Config, ProviderConfig, ProviderKind};
use permesh_core::Snapshot;
use permesh_provider_sdk::{Metadata, Provider, ProviderError};
use permesh_secrets::{SecretRef, SecretResolver};
use std::{collections::VecDeque, sync::Arc, time::Duration};
const MAX_CONCURRENT_PROVIDERS: usize = 4;
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(120);
pub fn kind(provider: &ProviderConfig) -> &'static str {
    match provider.kind {
        ProviderKind::Demo => "demo",
        ProviderKind::Github => "github",
        ProviderKind::Google => "google",
    }
}
pub fn metadata(provider: &ProviderConfig) -> Metadata {
    match provider.kind {
        ProviderKind::Demo => permesh_provider_demo::provider_metadata(),
        ProviderKind::Github => permesh_provider_github::provider_metadata(),
        ProviderKind::Google => permesh_provider_google::provider_metadata(),
    }
}
pub fn build(provider: &ProviderConfig) -> Result<Arc<dyn Provider>, ProviderError> {
    match provider.kind {
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
    providers: Vec<ProviderConfig>,
    discovery: bool,
    command: &str,
) -> Result<(Outcome, Vec<Snapshot>), AppError> {
    let mut outcome = Outcome::new(command, serde_json::json!({}))?;
    let mut pending: VecDeque<_> = providers.into();
    let mut tasks = tokio::task::JoinSet::new();
    let mut snapshots = vec![];
    while !pending.is_empty() || !tasks.is_empty() {
        while tasks.len() < MAX_CONCURRENT_PROVIDERS {
            let Some(provider) = pending.pop_front() else {
                break;
            };
            let blocking = blocking.clone();
            tasks.spawn(async move {
                let id=provider.id.clone();let kind=kind(&provider).to_string();
                let result=tokio::time::timeout(PROVIDER_TIMEOUT,async {
                    let adapter = blocking.run(move || build(&provider)).await
                        .map_err(|_| ProviderError::new("native_io", "Cannot resolve provider credentials"))??;
                    if discovery {
                        let mut snapshot=adapter.discover().await?;
                        if snapshot.provider!=id {return Err(ProviderError::new("invalid_snapshot","Provider returned a mismatched instance ID"));}
                        permesh_provider_sdk::validate_snapshot(&snapshot)?;snapshot.sort();
                        Ok(("Discovery completed".to_string(),snapshot.limitations.clone(),Some(snapshot)))
                    }else{let health=adapter.check().await?;Ok((health.message,health.limitations,None))}
                }).await.unwrap_or_else(|_|Err(ProviderError::new("timeout","Provider exceeded the 120-second deadline. Check connectivity or reduce configured organizations.")));
                (id,kind,result)
            });
        }
        let Some(joined) = tasks.join_next().await else {
            break;
        };
        let (id, kind, result) = joined.map_err(|_| {
            AppError::new(
                5,
                "Provider task failed unexpectedly; no diagnostic payload was retained",
            )
        })?;
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
