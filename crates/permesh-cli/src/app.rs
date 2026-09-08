// SPDX-License-Identifier: MIT
use crate::{
    args::{Cli, Command, ProviderCommand},
    auth, collection,
    error::AppError,
    report::Outcome,
    workspace,
};
pub async fn run(
    cli: &Cli,
    blocking: &crate::blocking::BlockingPool,
    cancellation: &crate::cancellation::Cancellation,
) -> Result<Outcome, AppError> {
    match &cli.command {
        Command::Provider {
            command: ProviderCommand::Migrate(args),
        } => {
            return crate::provider_migration::run(cli, args, blocking, cancellation).await;
        }
        Command::Provider {
            command: ProviderCommand::Install(args),
        } => {
            return crate::distribution::run(
                &args.provider,
                Some(&args.version),
                false,
                false,
                blocking,
                cancellation,
            )
            .await;
        }
        Command::Provider {
            command: ProviderCommand::Update(args),
        } => {
            return crate::distribution::run(
                &args.provider,
                args.version.as_deref(),
                true,
                args.check,
                blocking,
                cancellation,
            )
            .await;
        }
        _ => (),
    }
    if let Command::Provider {
        command: ProviderCommand::Setup(args),
    } = &cli.command
    {
        return crate::setup::run(cli, args, blocking, cancellation).await;
    }
    if let Command::Init { demo, organization } = &cli.command {
        let owned_cli = cli.clone();
        let demo = *demo;
        let organization = organization.clone();
        return blocking
            .run(move || workspace::init(&owned_cli, demo, &organization))
            .await?;
    }
    if let Command::Version = &cli.command {
        return Outcome::new(
            "version",
            serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"telemetry":false,"backend":false}),
        );
    }
    if let Command::Provider {
        command: ProviderCommand::Add(args),
    } = &cli.command
    {
        return workspace::add(cli, args);
    }
    let path = std::fs::canonicalize(workspace::path(cli)?)
        .map_err(|_| AppError::input("Cannot locate workspace configuration"))?;
    let config = permesh_config::Config::load(&path)?;
    match &cli.command {
        Command::Auth {
            command:
                crate::args::AuthCommand::Login {
                    id,
                    browser: true,
                    no_open,
                    ..
                },
        } => crate::browser_login::run(&config, &path, id, *no_open, blocking, cancellation).await,
        Command::Auth { command } => auth::run(&config, command, cli.json, blocking).await,
        Command::Provider {
            command: ProviderCommand::List,
        } => {
            let mut providers: Vec<_> = config
                .providers
                .iter()
                .map(|p| serde_json::json!({"id":p.id,"type":collection::kind(p)}))
                .collect();
            providers.sort_by_key(|v| v["id"].as_str().unwrap_or_default().to_string());
            Outcome::new("provider_list", serde_json::json!({"providers":providers}))
        }
        Command::Provider {
            command: ProviderCommand::Capabilities { id },
        } => {
            let provider = config
                .providers
                .iter()
                .find(|p| p.id == *id)
                .ok_or_else(|| {
                    AppError::input("Unknown provider instance; run permesh provider list")
                })?;
            Outcome::new(
                "provider_capabilities",
                serde_json::json!({"id":id,"metadata":collection::metadata(provider)?}),
            )
        }
        Command::Doctor
        | Command::Provider {
            command: ProviderCommand::Status { .. },
        } => {
            let id = if let Command::Provider {
                command: ProviderCommand::Status { id },
            } = &cli.command
            {
                id.as_deref()
            } else {
                None
            };
            let (mut outcome, _) = collection::collect(
                blocking,
                &config,
                &path,
                cancellation,
                collection::selected(&config, id)?,
                false,
                if matches!(cli.command, Command::Doctor) {
                    "doctor"
                } else {
                    "provider_status"
                },
            )
            .await?;
            outcome.report.result = serde_json::json!({"workspace_schema":config.version,"organization":config.organization.name,"identity_sources":config.identity.sources,"message":if config.providers.is_empty(){"Configuration valid. No providers configured; run permesh provider install github --version VERSION."}else{"Configuration valid. Health checks do not enumerate the access graph. Use a query to test discovery visibility."},"external_providers":"External execution requires a pinned registered binary and approval of this workspace configuration","local_overrides":"Not loaded"});
            Ok(outcome)
        }
        Command::User { .. } | Command::Admins | Command::Orphaned => {
            if config.providers.is_empty() {
                return Err(AppError::input(
                    "No providers configured. Run permesh provider install github --version VERSION, or try a new workspace with init --demo.",
                ));
            }
            let authorities: Vec<_> = config
                .identity
                .sources
                .iter()
                .filter(|source| source.authoritative)
                .map(|source| source.provider.clone())
                .collect();
            if matches!(cli.command, Command::Orphaned) && authorities.is_empty() {
                return Err(AppError::input(
                    "Orphaned account review requires an explicitly configured authoritative identity source. Configure identity.sources or add Google with --authoritative.",
                ));
            }
            let command = match cli.command {
                Command::Admins => "admins",
                Command::Orphaned => "orphaned",
                _ => "user",
            };
            let (mut outcome, snapshots) = collection::collect(
                blocking,
                &config,
                &path,
                cancellation,
                config.providers.clone(),
                true,
                command,
            )
            .await?;
            if snapshots.is_empty() {
                return Ok(outcome);
            }
            if matches!(cli.command, Command::Orphaned) {
                let result = permesh_core::query_orphaned(
                    &snapshots,
                    &config.identity.aliases,
                    &authorities,
                )?;
                outcome.report.result = serde_json::to_value(result)
                    .map_err(|_| AppError::new(5, "Cannot serialize orphaned account review"))?;
                return Ok(outcome);
            }
            if matches!(cli.command, Command::Admins) {
                let result = permesh_core::query_admins(&snapshots, &config.identity.aliases)?;
                outcome.report.result = serde_json::to_value(result)
                    .map_err(|_| AppError::new(5, "Cannot serialize privileged-access result"))?;
                return Ok(outcome);
            }
            let Command::User { identity } = &cli.command else {
                return Err(AppError::new(5, "Query command dispatch failed"));
            };
            match permesh_core::query_user(&snapshots, &config.identity.aliases, identity) {
                Ok(result) => {
                    outcome.report.result = serde_json::to_value(result)
                        .map_err(|_| AppError::new(5, "Cannot serialize query result"))?;
                }
                Err(permesh_core::DomainError::NotFound) if !outcome.report.complete => {
                    outcome.report.result = serde_json::json!({"identity":null,"accounts":[],"access":[],"message":"No match in available results; unavailable providers may contain this identity."});
                }
                Err(error) => return Err(error.into()),
            }
            Ok(outcome)
        }
        _ => Err(AppError::new(5, "Command dispatch failed")),
    }
}
