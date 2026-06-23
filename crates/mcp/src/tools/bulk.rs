//! Bulk command tools: start, stop, or restart every command in the session's project at
//! once. Trusted commands only; agents and terminals are untouched.

use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::server::SoloistMcp;
use crate::tools::reply::{acked, app_error, structured, unexpected};

#[tool_router(router = bulk_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Start every trusted command in this session's project (whatever its auto-start setting). Returns the ids that started and any skipped as untrusted. Agents and terminals are untouched."
    )]
    pub(crate) async fn start_all_commands(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::StartAllCommands).await {
            Ok(IpcResponse::BulkStarted(summary)) => structured(&summary),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Gracefully stop every running command in this session's project. Leaves agents and terminals running. Returns how many commands were stopped."
    )]
    pub(crate) async fn stop_all_commands(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::StopAllCommands).await {
            Ok(IpcResponse::BulkStopped(stopped)) => {
                structured(&serde_json::json!({ "stopped": stopped }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Restart every trusted command in this session's project, bringing the command set up fresh: running ones cycle, stopped ones start. Untrusted commands are skipped."
    )]
    pub(crate) async fn restart_all_commands(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::RestartAllCommands).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
