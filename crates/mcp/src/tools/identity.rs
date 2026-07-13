//! Identity & session tools: who this session is and what its scoped tools act on.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::{ProcessId, ProjectId};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{ProcessArg, RegisterAgentArg, SelectProjectArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{acked, app_error, structured, unexpected};

/// What `whoami`'s `mcp_tools.visibility_note` tells the caller: the count is the server's own
/// enabled-tool total, so a client showing fewer needs to refresh discovery or reconnect.
const MCP_TOOLS_VISIBILITY_NOTE: &str =
    "Soloist's server-side count of enabled MCP tools. If your \
MCP client shows fewer Soloist tools, refresh tool discovery or reconnect the Soloist MCP server.";

#[tool_router(router = identity_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Report which process this MCP session is bound to, who it is acting as, the project its scoped tools act on (by name), and how many Soloist MCP tools are enabled server-side. Call this first to confirm your identity and scope."
    )]
    pub(crate) async fn whoami(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::Whoami).await {
            // The identity and scope come from the core; the enabled-tool count is a fact of this
            // server's own composed surface, so it is attached here rather than round-tripped.
            Ok(IpcResponse::Whoami(who)) => {
                let mut value = serde_json::to_value(&who)
                    .map_err(|err| ErrorData::internal_error(err.to_string(), None))?;
                if let Some(object) = value.as_object_mut() {
                    object.insert("mcp_tools".into(), self.mcp_tools_status());
                }
                structured(&value)
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    /// The `mcp_tools` block `whoami` reports: that tools are enabled, the server-side enabled-tool
    /// count, and the note explaining a client-side discrepancy.
    fn mcp_tools_status(&self) -> serde_json::Value {
        serde_json::json!({
            "enabled": true,
            "server_enabled_tool_count": self.served_tool_count(),
            "visibility_note": MCP_TOOLS_VISIBILITY_NOTE,
        })
    }

    #[tool(
        description = "Register this MCP session as an external caller under a label, so `whoami` reports who is calling. For agents Soloist did not launch."
    )]
    pub(crate) async fn register_agent(
        &self,
        Parameters(RegisterAgentArg { label }): Parameters<RegisterAgentArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::RegisterAgent { label })
            .await
        {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Set the project this session's scoped tools act on, by its id from `list_projects`."
    )]
    pub(crate) async fn select_project(
        &self,
        Parameters(SelectProjectArg { project }): Parameters<SelectProjectArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SelectProject {
            project: ProjectId::from_raw(project),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Note a process as this session's default target, by its id. Informational only — it is reported by `whoami` and confers no scope; every tool still takes an explicit process id."
    )]
    pub(crate) async fn select_process(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SelectProcess {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
