//! Agent tools: spawning a worker agent in the session's project and listing the agent tools
//! that can be spawned.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{ReportToLeadArg, SpawnAgentArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = agent_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Spawn a configured agent tool as a worker in this session's project and start it. Use `list_agent_tools` for the available names. Returns the new process id. Delegation is one level deep: a worker spawned by a lead cannot itself spawn agents. Set `close_when_done` to remove the worker from the process list once its run ends on its own and it has reported back to you; a worker you stop, one that crashes, and one that never reported all keep their row and their output so nothing is lost. It defaults to false, which leaves every finished worker there with its output readable."
    )]
    pub(crate) async fn spawn_agent(
        &self,
        Parameters(SpawnAgentArg {
            tool,
            extra_args,
            close_when_done,
        }): Parameters<SpawnAgentArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SpawnAgent {
            tool,
            extra_args,
            close_when_done,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Spawned(id)) => structured(&serde_json::json!({ "process": id })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Hand your final result to the lead agent that spawned you, delivered as a fresh turn on its terminal so it wakes with what you found. This is how a worker signals it is done, and the only way: going quiet does not, because a worker that has finished and one that is still thinking look identical from outside. Call it when your task is complete, including when you failed or finished only part of it. You cannot choose who receives it: the lead is resolved from who spawned you, and a caller no agent spawned has none to report to."
    )]
    pub(crate) async fn report_to_lead(
        &self,
        Parameters(ReportToLeadArg { report }): Parameters<ReportToLeadArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .client
            .request(IpcRequest::ReportToLead { report })
            .await
        {
            Ok(IpcResponse::Acked) => structured(&serde_json::json!({ "delivered": true })),
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
