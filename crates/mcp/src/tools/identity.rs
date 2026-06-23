//! Identity & session tools: who this session is and what its scoped tools act on.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::{ProcessId, ProjectId};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{ProcessArg, RegisterAgentArg, SelectProjectArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{acked, app_error, structured, unexpected};

#[tool_router(router = identity_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Report which process this MCP session is bound to and the project its scoped tools act on."
    )]
    pub(crate) async fn whoami(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::Whoami).await {
            Ok(IpcResponse::Whoami(who)) => structured(&who),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
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
