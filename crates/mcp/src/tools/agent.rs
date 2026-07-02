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
        description = "Spawn a configured agent tool as a worker in this session's project and start it. Use `list_agent_tools` for the available names. Returns the new process id. Delegation is one level deep: a worker spawned by a lead cannot itself spawn agents."
    )]
    pub(crate) async fn spawn_agent(
        &self,
        Parameters(SpawnAgentArg { tool, extra_args }): Parameters<SpawnAgentArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SpawnAgent { tool, extra_args };
        match self.client.request(request).await {
            Ok(IpcResponse::Spawned(id)) => structured(&serde_json::json!({ "process": id })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "List the configured agent tools that `spawn_agent` can launch.")]
    pub(crate) async fn list_agent_tools(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListAgentTools).await {
            Ok(IpcResponse::AgentTools(tools)) => structured(&tools),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
