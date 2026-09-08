// SPDX-License-Identifier: MIT OR Apache-2.0
mod admins_output;
mod app;
mod args;
mod auth;
mod blocking;
mod cancellation;
mod collection;
mod completion;
mod error;
mod external;
mod external_output;
mod external_workspace;
mod orphaned_output;
mod output;
mod provider_operation;
mod report;
mod setup;
mod setup_output;
mod setup_prompt;
mod setup_workspace;
mod workspace;
use clap::Parser;
use error::AppError;
use std::process::ExitCode;
#[tokio::main]
async fn main() -> ExitCode {
    let raw: Vec<_> = std::env::args_os().collect();
    let json = raw.iter().any(|s| s == "--json");
    let cli = match args::Cli::try_parse_from(raw) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                return if error.print().is_ok() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(5)
                };
            }
            let curated = AppError::input(
                "Invalid command arguments. Run permesh --help or permesh <command> --help for usage.",
            );
            return finish_error(curated, json);
        }
    };
    if let args::Command::Completion { shell } = &cli.command {
        // Global flags placed before a subcommand need an explicit check.
        if cli.json {
            return finish_error(
                AppError::input(
                    "Shell completion emits a script and cannot be combined with --json",
                ),
                true,
            );
        }
        return match completion::write(*shell, &mut std::io::stdout().lock()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
            Err(_) => finish_error(
                AppError::new(5, "Cannot write shell completion script"),
                false,
            ),
        };
    }
    let blocking = blocking::BlockingPool::new();
    let result = if let args::Command::Provider {
        command: args::ProviderCommand::External { command },
    } = &cli.command
    {
        external::run(command, &cli, &blocking).await
    } else {
        let cancellation = cancellation::Cancellation::new();
        let operation = app::run(&cli, &blocking, &cancellation);
        tokio::pin!(operation);
        tokio::select! {
            biased;
            signal=tokio::signal::ctrl_c()=>{
                cancellation.cancel();
                let drains = matches!(cli.command, args::Command::Doctor | args::Command::User { .. } | args::Command::Admins | args::Command::Orphaned | args::Command::Provider { command: args::ProviderCommand::Status { .. } | args::ProviderCommand::Setup(_) });
                if drains {
                    let result = operation.await;
                    if let Err(error) = result && error.code == 5 { return finish_error(error, cli.json); }
                }
                if signal.is_ok() { Err(AppError::new(130,"Cancelled")) } else { Err(AppError::new(5,"Cannot install Ctrl+C handler")) }
            },
            result=&mut operation=>result,
        }
    };
    match result {
        Ok(outcome) => {
            match output::write_report(&outcome.report, cli.json, cli.color, cli.verbose) {
                Ok(()) => ExitCode::from(outcome.code),
                Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(5),
            }
        }
        Err(error) => finish_error(error, cli.json),
    }
}
fn finish_error(error: AppError, json: bool) -> ExitCode {
    match output::write_error(&error, json) {
        Ok(()) => ExitCode::from(error.code),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(5),
    }
}
