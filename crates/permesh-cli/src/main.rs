// SPDX-License-Identifier: MIT OR Apache-2.0
mod admins_output;
mod app;
mod args;
mod auth;
mod blocking;
mod collection;
mod error;
mod orphaned_output;
mod output;
mod report;
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
    let blocking = blocking::BlockingPool::new();
    let result = tokio::select! {
        biased;
        signal=tokio::signal::ctrl_c()=>match signal {Ok(())=>Err(AppError::new(130,"Cancelled")),Err(_)=>Err(AppError::new(5,"Cannot install Ctrl+C handler"))},
        result=app::run(&cli, &blocking)=>result,
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
