//! The rmcp server handler: the read-only tools, each a thin call to one IPC request.
//!
//! Tool *names* mirror Solo for interop, but the parameter schemas are clean-room — derived
//! from the argument structs here. No domain logic lives in a tool: each forwards to the app,
//! which resolves identity, scope, and the trust gate in the core, and the result is returned
//! as structured content.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler};
use serde::{Deserialize, Serialize};
use soloist_core::{ProcessId, ProjectId};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::client::{AppClient, ClientError};

/// The Soloist MCP server: a stateless front over the app, holding one client connection.
#[derive(Clone)]
pub struct SoloistMcp {
    client: Arc<AppClient>,
    tool_router: ToolRouter<Self>,
}

/// Arguments for a single-process tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ProcessArg {
    /// The id of the process, as returned by `list_processes`.
    process: u64,
}

/// Arguments for a project-scoped tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ProjectArg {
    /// The id of the project. Omit to use the session's effective project scope.
    project: Option<u64>,
}

#[tool_router]
impl SoloistMcp {
    /// Builds the handler over a client connection to the app.
    pub fn new(client: Arc<AppClient>) -> Self {
        Self {
            client,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Report which process this MCP session is bound to and the project its scoped tools act on."
    )]
    async fn whoami(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::Whoami).await {
            Ok(IpcResponse::Whoami(who)) => structured(&who),
            Ok(_) => Err(unexpected()),
            Err(err) => Err(app_error(&err)),
        }
    }

    #[tool(description = "List every project Soloist has open.")]
    async fn list_projects(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListProjects).await {
            Ok(IpcResponse::Projects(projects)) => structured(&projects),
            Ok(_) => Err(unexpected()),
            Err(err) => Err(app_error(&err)),
        }
    }

    #[tool(
        description = "Get a project and its processes. Omit `project` to use the session's effective project."
    )]
    async fn get_project_status(
        &self,
        Parameters(ProjectArg { project }): Parameters<ProjectArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GetProjectStatus {
            project: project.map(ProjectId::from_raw),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::ProjectStatus(status)) => structured(&status),
            Ok(_) => Err(unexpected()),
            Err(err) => Err(app_error(&err)),
        }
    }

    #[tool(description = "List every process Soloist supervises across all projects.")]
    async fn list_processes(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListProcesses).await {
            Ok(IpcResponse::Processes(processes)) => structured(&processes),
            Ok(_) => Err(unexpected()),
            Err(err) => Err(app_error(&err)),
        }
    }

    #[tool(description = "Get one process's current status by its id.")]
    async fn get_process_status(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GetProcessStatus {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Process(view)) => structured(&view),
            Ok(_) => Err(unexpected()),
            Err(err) => Err(app_error(&err)),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for SoloistMcp {}

/// Serializes a reply into a structured tool result.
fn structured<T: Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    serde_json::to_value(value)
        .map(CallToolResult::structured)
        .map_err(|err| ErrorData::internal_error(err.to_string(), None))
}

/// Maps a client error to an MCP tool error the agent can read.
fn app_error(err: &ClientError) -> ErrorData {
    ErrorData::internal_error(err.to_string(), None)
}

/// The app returned a response of the wrong shape — a protocol mismatch, not a user error.
fn unexpected() -> ErrorData {
    ErrorData::internal_error("the app returned an unexpected response".to_string(), None)
}
