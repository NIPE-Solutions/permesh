// SPDX-License-Identifier: MIT
//! Read-only MCP server over stdio (ADR 0035). Tools run existing commands only.
mod paths;
mod server;
mod tools;
use crate::{args::Cli, blocking::BlockingPool, error::AppError, workspace};
use rmcp::{ServiceExt, service::ServerInitializeError};

pub(crate) async fn serve(cli: &Cli, blocking: &BlockingPool) -> Result<(), AppError> {
    let workspace = std::fs::canonicalize(workspace::path(cli)?)
        .map_err(|_| AppError::input("Cannot locate workspace configuration"))?;
    permesh_config::Config::load(&workspace)?;
    let handler = server::Server::new(workspace, blocking.clone());
    let session = tokio::select! {
        session = handler.serve(rmcp::transport::stdio()) => session,
        _ = tokio::signal::ctrl_c() => return Err(AppError::new(130, "Cancelled")),
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
    tokio::select! {
        result = running.waiting() => result
            .map(|_| ())
            .map_err(|_| AppError::new(5, "The MCP session ended unexpectedly")),
        _ = tokio::signal::ctrl_c() => Err(AppError::new(130, "Cancelled")),
    }
}
