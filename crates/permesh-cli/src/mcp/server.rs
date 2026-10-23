// SPDX-License-Identifier: MIT
use super::tools;
use crate::{
    app, args::Cli, blocking::BlockingPool, cancellation::Cancellation, error::AppError,
    report::Outcome,
};
use clap::Parser;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, Implementation, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
};
use std::{borrow::Cow, path::PathBuf, sync::Arc};

pub(crate) const INSTRUCTIONS: &str = "Permesh answers who has access to what, read-only. Every tool result has `complete`: when it is false, some providers could not be fully read and missing access is not proof of absence, so say you cannot tell. Plan tools write a new plan file for a person to review and carry out; Permesh never changes access.";
const VERSIONS: &[ProtocolVersion] = &[ProtocolVersion::V_2026_07_28];

pub(crate) struct Server {
    workspace: PathBuf,
    blocking: BlockingPool,
    /// One command at a time: discoveries never share provider sessions or the keyring.
    /// The session takes it after shutdown to wait for the running call's drain.
    lock: Arc<tokio::sync::Mutex<()>>,
    /// Cancelled when the session ends (stdin closed, Ctrl+C or SIGTERM).
    shutdown: Cancellation,
}
impl Server {
    pub(crate) fn new(
        workspace: PathBuf,
        blocking: BlockingPool,
        lock: Arc<tokio::sync::Mutex<()>>,
        shutdown: Cancellation,
    ) -> Self {
        Self {
            workspace,
            blocking,
            lock,
            shutdown,
        }
    }
    async fn execute(
        &self,
        tool: &tools::Tool,
        stop: impl std::future::Future<Output = ()>,
    ) -> Result<Outcome, AppError> {
        let cli = Cli::try_parse_from(tool.argv(&self.workspace)?)
            .map_err(|_| AppError::input("Invalid tool arguments"))?;
        if self.shutdown.is_cancelled() {
            return Err(AppError::new(130, "Cancelled"));
        }
        let cancellation = Cancellation::new();
        let operation = app::run(&cli, &self.blocking, &cancellation);
        tokio::pin!(operation);
        tokio::select! {
            biased;
            () = async {
                tokio::select! {
                    () = stop => {},
                    () = self.shutdown.cancelled() => {},
                }
            } => {
                // Drain like Ctrl+C in main: providers are terminated and reaped first.
                cancellation.cancel();
                match operation.await {
                    Err(error) if error.code == 5 => Err(error),
                    _ => Err(AppError::new(130, "Cancelled")),
                }
            }
            result = &mut operation => result,
        }
    }
}
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        info.server_info = Implementation::new("permesh", env!("CARGO_PKG_VERSION"));
        info.instructions = Some(INSTRUCTIONS.into());
        info
    }
    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(VERSIONS)
    }
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools::definitions()))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let arguments = request.arguments.unwrap_or_default();
        let outcome = match tools::parse(&request.name, arguments) {
            Ok(tool) => {
                let _turn = self.lock.lock().await;
                self.execute(&tool, context.ct.cancelled()).await
            }
            Err(message) => Err(AppError::input(message)),
        };
        Ok(CallToolResponse::Complete(tools::result(outcome)))
    }
}
