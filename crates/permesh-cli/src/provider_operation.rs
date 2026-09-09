// SPDX-License-Identifier: MIT
use crate::provider_diagnostics::Code;
use crate::{blocking::BlockingPool, cancellation::Cancellation, error::AppError};
use permesh_config::{Config, DiscoveryProtocol, ProviderConfig, ProviderKind};
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
    if matches!(provider.kind, ProviderKind::Github | ProviderKind::Google) {
        return Err(
            AppError::new(3, crate::collection::legacy_message(&provider))
                .diagnostic(Code::MigrationRequired),
        );
    }
    let id = provider.id.clone();
    let observation = if provider.kind == ProviderKind::Inventory {
        let path = path.to_owned();
        let observed = tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(AppError::new(130, "Cancelled")),
            result = within_deadline(PROVIDER_TIMEOUT, pool.run(move || crate::inventory::observe(&provider, &path)), "Inventory read exceeded the 120-second deadline") => result???,
        };
        if !discovery && !observed.snapshot.complete {
            return Err(AppError::new(
                3,
                "Inventory is stale or explicitly incomplete; review and pin a fresh complete export before authority assessment",
            ));
        }
        ("Pinned local inventory read; declared lifecycle does not establish employment or absence outside its scope".into(), observed.snapshot.limitations.clone(), discovery.then_some(observed.snapshot))
    } else if provider.kind == ProviderKind::External {
        let protocol = provider
            .external
            .as_ref()
            .ok_or_else(|| AppError::input("External provider configuration is missing"))?
            .discovery_protocol;
        let config = config.clone();
        let path = path.to_owned();
        let prepared = tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(AppError::new(130, "Cancelled")),
            result = within_deadline(PROVIDER_TIMEOUT, pool.run(move || crate::external_workspace::prepare(&config, &path, &provider)), "External credential preparation exceeded the 120-second deadline") => result???,
        };
        let (executable, registration, invocation) = prepared;
        // The supervisor owns its deadlines. Never drop it before process cleanup finishes.
        if discovery {
            let snapshot = match protocol {
                DiscoveryProtocol::Legacy => {
                    host::discover_configured(
                        &executable,
                        &registration.id,
                        &id,
                        &registration.capabilities,
                        &invocation,
                        cancellation.cancelled(),
                    )
                    .await
                }
                DiscoveryProtocol::NegotiatedV1 => {
                    host::discover_negotiated(
                        &executable,
                        &registration.id,
                        &id,
                        &registration.capabilities,
                        &invocation,
                        cancellation.cancelled(),
                    )
                    .await
                }
            }
            .map_err(external_error)?;
            (
                "Discovery completed".into(),
                snapshot.limitations.clone(),
                Some(snapshot),
            )
        } else {
            let health = match protocol {
                DiscoveryProtocol::Legacy => {
                    host::check_configured(
                        &executable,
                        &registration.id,
                        &id,
                        &registration.capabilities,
                        &invocation,
                        cancellation.cancelled(),
                    )
                    .await
                }
                DiscoveryProtocol::NegotiatedV1 => {
                    host::check_negotiated(
                        &executable,
                        &registration.id,
                        &id,
                        &registration.capabilities,
                        &invocation,
                        cancellation.cancelled(),
                    )
                    .await
                }
            }
            .map_err(external_error)?;
            (health.message, health.limitations, None)
        }
    } else {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => return Err(AppError::new(130, "Cancelled")),
            result = within_deadline(PROVIDER_TIMEOUT, async {
                let adapter = pool.run(move || crate::collection::build(&provider)).await??;
                if discovery {
                    let snapshot = adapter.discover().await?;
                    Ok::<_, AppError>(("Discovery completed".into(), snapshot.limitations.clone(), Some(snapshot)))
                } else {
                    let health = adapter.check().await?;
                    Ok((health.message, health.limitations, None))
                }
            }, "Provider exceeded the 120-second deadline. Check connectivity or reduce configured organizations.") => result??,
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
    let code = Code::host(&error);
    crate::external::failure(error).diagnostic(code)
}

async fn within_deadline<T>(
    duration: Duration,
    operation: impl std::future::Future<Output = T>,
    message: &'static str,
) -> Result<T, AppError> {
    tokio::time::timeout(duration, operation)
        .await
        .map_err(|_| AppError::new(3, message).diagnostic(Code::DeadlineExceeded))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preparation_and_builtin_deadlines_preserve_typed_provenance() {
        for message in ["preparation deadline", "provider deadline"] {
            let result =
                within_deadline(Duration::ZERO, std::future::pending::<()>(), message).await;
            let Err(error) = result else {
                panic!("pending operation completed")
            };
            assert_eq!(error.code, 3);
            assert_eq!(error.diagnostic, Some(Code::DeadlineExceeded));
            assert_eq!(error.message, message);
        }
        assert!(
            within_deadline(Duration::from_secs(1), std::future::ready(42), "deadline")
                .await
                .is_ok_and(|value| value == 42)
        );
    }
}
