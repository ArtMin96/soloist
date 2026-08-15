//! Agent tools: spawning a worker agent in the session's project and listing the agent tools
//! that can be spawned.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::SpawnAgentArg;
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = agent_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Spawn a configured agent tool as a worker in this session's project and start it. Use `list_agent_tools` for the available names. An optional prompt is queued as the worker's initial task message, not added to its command line or pasted into its terminal. Compact Soloist coordination instructions are included by default; set include_agent_instructions=false only when the worker already has equivalent guidance. Returns the new process id and, when a prompt was supplied, its initial message id and delivery state. Delegation is one level deep: a worker spawned by a lead cannot itself spawn agents."
    )]
    pub(crate) async fn spawn_agent(
        &self,
        Parameters(SpawnAgentArg {
            tool,
            extra_args,
            prompt,
            include_agent_instructions,
        }): Parameters<SpawnAgentArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SpawnAgent {
            tool,
            extra_args,
            prompt,
            todo_id: None,
            include_agent_instructions,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Spawned(process)) => {
                structured(&serde_json::json!({ "process": process }))
            }
            Ok(IpcResponse::SpawnedWithMessage {
                process,
                initial_message_id,
                delivery,
            }) => structured(&serde_json::json!({
                "process": process,
                "initial_message_id": initial_message_id,
                "delivery": delivery,
            })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "List the configured agent tools that `spawn_agent` can launch.")]
    pub(crate) async fn list_agent_tools(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListAgentTools).await {
            // Wrapped in an object: the MCP spec requires `structuredContent` to be a JSON
            // object, so a list reply must never be a bare array (clients refuse it).
            Ok(IpcResponse::AgentTools(tools)) => {
                structured(&serde_json::json!({ "tools": tools }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
