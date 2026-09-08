// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{blocking::BlockingPool, cancellation::Cancellation, error::AppError};
use permesh_config::{Config, ProviderConfig, ProviderKind};
use permesh_core::Snapshot;
use permesh_provider_external::{ExternalError, host};
use std::{path::Path, time::Duration};

const PROVIDER_TIMEOUT: Duration = Duration::from_secs(120);
type Observation = (String, Vec<String>, Option<Snapshot>);

pub async fn run(
    pool: &BlockingPool,
    config: &Config,
    path: &Path,
    provider: ProviderConfig,
    discovery: bool,
    cancellation: &Cancellation,
) -> Result<Observation, AppError> {
    if cancellation.is_cancelled() {
        return Err(AppError::new(130, "Cancelled"));
    }
    let id = provider.id.clone();
    let observation = if provider.kind == ProviderKind::External {
        let config = config.clone();
        let path = path.to_owned();
        let prepared = tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(AppError::new(130, "Cancelled")),
            result = tokio::time::timeout(PROVIDER_TIMEOUT, pool.run(move || crate::external_workspace::prepare(&config, &path, &provider))) => result.map_err(|_| AppError::new(3, "External credential preparation exceeded the 120-second deadline"))???,
        };
        let (executable, registration, invocation) = prepared;
        // The supervisor owns its deadlines. Never drop it before process cleanup finishes.
        if discovery {
            let snapshot = host::discover_configured(
                &executable,
                &registration.id,
                &id,
                &registration.capabilities,
                &invocation,
                cancellation.cancelled(),
            )
            .await
            .map_err(external_error)?;
            (
                "Discovery completed".into(),
                snapshot.limitations.clone(),
                Some(snapshot),
            )
        } else {
            let health = host::check_configured(
                &executable,
                &registration.id,
                &id,
                &registration.capabilities,
                &invocation,
                cancellation.cancelled(),
            )
            .await
            .map_err(external_error)?;
            (health.message, health.limitations, None)
        }
    } else {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(AppError::new(130, "Cancelled")),
            result = tokio::time::timeout(PROVIDER_TIMEOUT, async {
                let adapter = pool.run(move || crate::collection::build(&provider)).await??;
                if discovery {
                    let snapshot = adapter.discover().await?;
                    Ok::<_, AppError>(("Discovery completed".into(), snapshot.limitations.clone(), Some(snapshot)))
                } else {
                    let health = adapter.check().await?;
                    Ok((health.message, health.limitations, None))
                }
            }) => result.map_err(|_| AppError::new(3, "Provider exceeded the 120-second deadline. Check connectivity or reduce configured organizations."))??,
        }
    };
    let (message, limitations, mut snapshot) = observation;
    if let Some(snapshot) = &mut snapshot {
        if snapshot.provider != id {
            return Err(AppError::new(
                3,
                "Provider returned a mismatched instance ID",
            ));
        }
        permesh_provider_sdk::validate_snapshot(snapshot)?;
        snapshot.sort();
    }
    Ok((message, limitations, snapshot))
}

fn external_error(error: ExternalError) -> AppError {
    if matches!(error, ExternalError::Cleanup) {
        return AppError::new(5, "External provider process cleanup failed");
    }
    crate::external::failure(error)
}
