// SPDX-License-Identifier: MIT
//! Read-only MCP server over stdio (ADR 0035). Tools run existing commands only.
mod paths;
mod server;
mod tools;
use crate::{
    args::Cli, blocking::BlockingPool, cancellation::Cancellation, error::AppError, workspace,
};
use rmcp::{ServiceExt, service::ServerInitializeError};
use std::sync::Arc;

/// Resolves on Ctrl+C, or on SIGTERM where it exists (how MCP clients stop servers).
async fn stopped() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

pub(crate) async fn serve(cli: &Cli, blocking: &BlockingPool) -> Result<(), AppError> {
    let workspace = std::fs::canonicalize(workspace::path(cli)?)
        .map_err(|_| AppError::input("Cannot locate workspace configuration"))?;
    permesh_config::Config::load(&workspace)?;
    let lock = Arc::new(tokio::sync::Mutex::new(()));
    let shutdown = Cancellation::new();
    let handler = server::Server::new(workspace, blocking.clone(), lock.clone(), shutdown.clone());
    let session = tokio::select! {
        session = handler.serve(rmcp::transport::stdio()) => session,
        () = stopped() => return Err(AppError::new(130, "Cancelled")),
    };
    let running = match session {
        Ok(running) => running,
        Err(ServerInitializeError::ConnectionClosed(_)) => return Ok(()),
        Err(_) => {
            return Err(AppError::input(
                "The MCP session could not start. permesh mcp supports MCP protocol 2026-07-28 only; update the assistant or its MCP client.",
            ));
        }
    };
    let ended = tokio::select! {
        result = running.waiting() => result
            .map(|_| ())
            .map_err(|_| AppError::new(5, "The MCP session ended unexpectedly")),
        () = stopped() => Err(AppError::new(130, "Cancelled")),
    };
    // A running call drains like Ctrl+C in the CLI: providers are stopped and reaped first.
    shutdown.cancel();
    let _drained = lock.lock().await;
    ended
}
