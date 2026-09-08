// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{
    blocking::BlockingPool,
    error::AppError,
    report::{Outcome, ProviderStatus, now},
};
use clap::Subcommand;
use permesh_provider_external::{
    ExternalError, host,
    trust::{Registry, inspect},
};
use permesh_provider_sdk::Capability;
use std::path::PathBuf;

#[derive(Clone, Subcommand)]
pub enum ExternalCommand {
    /// Review this workspace instance and its proposed credential delivery without executing code.
    Review { instance: String },
    /// Approve the exact reviewed workspace fingerprint for query execution.
    Approve {
        instance: String,
        #[arg(long)]
        fingerprint: String,
        #[arg(long, required = true)]
        accept_risk: bool,
    },
    /// Revoke this workspace instance's local execution approval.
    Revoke { instance: String },
    /// Read a native executable's digest without executing it.
    Inspect { executable: PathBuf },
    /// Copy reviewed native code into local trust storage (no execution).
    Trust {
        executable: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        sha256: String,
        #[arg(long, value_parser=["accounts","identities","resources","groups","memberships","grants"])]
        capability: Vec<String>,
        #[arg(
            long,
            required = true,
            help = "Acknowledge this code can access files and network as your user; it is not sandboxed"
        )]
        accept_risk: bool,
    },
    /// List locally registered provider binaries; never execute them.
    List,
    /// Remove a local trust registration and its managed executable copy.
    Remove { id: String },
    /// Execute a trusted provider and validate its observed access snapshot.
    Discover {
        id: String,
        #[arg(long, help = "Stable provider instance ID for this discovery")]
        instance: String,
    },
}
pub(crate) fn storage_root() -> Result<PathBuf, AppError> {
    if let Some(root) = std::env::var_os("PERMESH_DATA_DIR") {
        let root = PathBuf::from(root);
        if !root.is_absolute() {
            return Err(AppError::input(
                "PERMESH_DATA_DIR must be an absolute user-local directory",
            ));
        }
        return Ok(root.join("providers"));
    }
    use etcetera::BaseStrategy;
    let strategy = etcetera::base_strategy::choose_native_strategy().map_err(|_| {
        AppError::input("Cannot locate user-local provider storage; set PERMESH_DATA_DIR to an absolute private directory")
    })?;
    // Windows trust state belongs in Local AppData, not roaming profile data.
    #[cfg(windows)]
    let base = strategy.cache_dir().join("permesh").join("data");
    #[cfg(not(windows))]
    let base = strategy.data_dir().join("permesh");
    Ok(base.join("providers"))
}
fn capabilities(values: &[String]) -> Result<Vec<Capability>, AppError> {
    values
        .iter()
        .map(|value| match value.as_str() {
            "accounts" => Ok(Capability::Accounts),
            "identities" => Ok(Capability::Identities),
            "resources" => Ok(Capability::Resources),
            "groups" => Ok(Capability::Groups),
            "memberships" => Ok(Capability::Memberships),
            "grants" => Ok(Capability::Grants),
            _ => Err(AppError::input("Unknown external provider capability")),
        })
        .collect()
}
pub(crate) fn failure(error: ExternalError) -> AppError {
    let code = match error {
        ExternalError::Input | ExternalError::Trust | ExternalError::Storage => 2,
        ExternalError::Cancelled => 130,
        _ => 3,
    };
    AppError::new(code, error.to_string())
}
async fn local<T: Send + 'static>(
    pool: &BlockingPool,
    operation: impl FnOnce() -> Result<T, ExternalError> + Send + 'static,
) -> Result<T, AppError> {
    tokio::select! {
        result=pool.run(operation)=>result?.map_err(failure),
        signal=tokio::signal::ctrl_c()=>Err(if signal.is_ok(){AppError::new(130,"Cancelled")}else{AppError::new(5,"Cannot install Ctrl+C handler")}),
    }
}
pub async fn run(
    command: &ExternalCommand,
    cli: &crate::args::Cli,
    pool: &BlockingPool,
) -> Result<Outcome, AppError> {
    if matches!(
        command,
        ExternalCommand::Review { .. }
            | ExternalCommand::Approve { .. }
            | ExternalCommand::Revoke { .. }
    ) {
        let cli = cli.clone();
        let command = command.clone();
        let operation = pool.run(move || {
            let path = crate::workspace::path(&cli)?;
            if let ExternalCommand::Revoke { instance } = command {
                return crate::external_workspace::revoke(&path, &instance);
            }
            let config = permesh_config::Config::load(&path)?;
            match command {
                ExternalCommand::Review { instance } => {
                    crate::external_workspace::review(&config, &path, &instance)
                }
                ExternalCommand::Approve {
                    instance,
                    fingerprint,
                    accept_risk,
                } => crate::external_workspace::approve(
                    &config,
                    &path,
                    &instance,
                    &fingerprint,
                    accept_risk,
                ),
                _ => Err(AppError::new(
                    5,
                    "External workspace command dispatch failed",
                )),
            }
        });
        return tokio::select! {
            result = operation => result?,
            signal = tokio::signal::ctrl_c() => Err(if signal.is_ok() { AppError::new(130, "Cancelled") } else { AppError::new(5, "Cannot install Ctrl+C handler") }),
        };
    }
    if let ExternalCommand::Inspect { executable } = command {
        let executable = executable.clone();
        let result = local(pool, move || inspect(&executable)).await?;
        return Outcome::new(
            "external_inspect",
            serde_json::json!({"inspection":result,"message":"Digest inspection only; no code was executed or trusted."}),
        );
    }
    let root = storage_root()?;
    let registry = Registry::new(root.clone()).map_err(failure)?;
    match command.clone() {
        ExternalCommand::Trust {
            executable,
            id,
            sha256,
            capability,
            accept_risk,
        } => {
            if !accept_risk {
                return Err(AppError::input(
                    "Trust requires --accept-risk after reviewing the binary and digest",
                ));
            }
            let caps = capabilities(&capability)?;
            let registration = local(pool, move || {
                registry.trust(&executable, &id, &sha256, &caps)
            })
            .await?;
            Outcome::new(
                "external_trust",
                serde_json::json!({"registration":registration,"storage":root,"message":"Native provider registered locally. This code is not sandboxed; discovery executes it with your user privileges."}),
            )
        }
        ExternalCommand::List => {
            let registrations = local(pool, move || registry.list()).await?;
            Outcome::new(
                "external_list",
                serde_json::json!({"registrations":registrations,"storage":root}),
            )
        }
        ExternalCommand::Remove { id } => {
            let removed = id.clone();
            local(pool, move || registry.remove(&id)).await?;
            Outcome::new(
                "external_remove",
                serde_json::json!({"id":removed,"storage":root,"message":"Local registration and managed binary removed. Provider-side state was not changed."}),
            )
        }
        ExternalCommand::Discover { id, instance } => {
            let mut outcome = Outcome::new("external_discover", serde_json::json!({}))?;
            let (registration, executable) = local(pool, move || {
                let registration = registry.load(&id)?;
                let executable = registry.verify(&registration)?;
                Ok((registration, executable))
            })
            .await?;
            let snapshot = {
                let cancellation = tokio::sync::Notify::new();
                let discover = host::discover(
                    &executable,
                    &registration.id,
                    &instance,
                    &registration.capabilities,
                    cancellation.notified(),
                );
                tokio::pin!(discover);
                tokio::select! {
                    result=&mut discover=>result.map_err(failure)?,
                    signal=tokio::signal::ctrl_c()=>{
                        cancellation.notify_one();
                        let cleanup=discover.await;
                        if signal.is_err(){return Err(AppError::new(5,"Cannot install Ctrl+C handler"));}
                        match cleanup {
                            Err(ExternalError::Cleanup) => return Err(failure(ExternalError::Cleanup)),
                            _ => return Err(AppError::new(130, "Cancelled")),
                        }
                    }
                }
            };
            outcome.report.complete = snapshot.complete;
            outcome.code = if snapshot.complete { 0 } else { 4 };
            outcome.report.providers.push(ProviderStatus {
                id: instance,
                kind: registration.id,
                state: if snapshot.complete {
                    "connected"
                } else {
                    "partial"
                }
                .into(),
                message: "Trusted external discovery completed".into(),
                limitations: snapshot.limitations.clone(),
            });
            outcome.report.result = serde_json::json!({"snapshot":snapshot,"storage":root});
            outcome.report.completed_at = now()?;
            Ok(outcome)
        }
        ExternalCommand::Inspect { .. }
        | ExternalCommand::Review { .. }
        | ExternalCommand::Approve { .. }
        | ExternalCommand::Revoke { .. } => {
            Err(AppError::new(5, "External command dispatch failed"))
        }
    }
}
