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

/// Arguments for writing input to a process.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SendInputArg {
    /// The id of the process to write to, as returned by `list_processes`.
    process: u64,
    /// The bytes to write to the process's input, as text. Control characters are sent
    /// verbatim — e.g. a trailing carriage return to submit a line, or 0x03 for Ctrl-C.
    input: String,
    /// Optionally wait this many milliseconds after writing, then return the rendered
    /// terminal tail so you can see the effect. Capped by the app; omit to return at once.
    wait_ms: Option<u64>,
}

/// Arguments for selecting the session's project scope.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SelectProjectArg {
    /// The id of the project to scope this session's tools to, from `list_projects`.
    project: u64,
}

/// Arguments for registering an external caller.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RegisterAgentArg {
    /// A short label identifying the calling agent (e.g. `claude-code`), reported by `whoami`.
    label: String,
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

    #[tool(
        description = "Register this MCP session as an external caller under a label, so `whoami` reports who is calling. For agents Soloist did not launch."
    )]
    async fn register_agent(
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
            Err(err) => Err(app_error(&err)),
        }
    }

    #[tool(
        description = "Set the project this session's scoped tools act on, by its id from `list_projects`."
    )]
    async fn select_project(
        &self,
        Parameters(SelectProjectArg { project }): Parameters<SelectProjectArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SelectProject {
            project: ProjectId::from_raw(project),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
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

    #[tool(
        description = "Start one process by its id. Acts only within the session's project; refused if the command is untrusted."
    )]
    async fn start_process(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::StartProcess {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => Err(app_error(&err)),
        }
    }

    #[tool(
        description = "Gracefully stop one process by its id. Reports whether it was running. Acts only within the session's project."
    )]
    async fn stop_process(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::StopProcess {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Stopped(was_running)) => {
                structured(&serde_json::json!({ "was_running": was_running }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => Err(app_error(&err)),
        }
    }

    #[tool(
        description = "Restart one process by its id (stop then start with its saved config). Acts only within the session's project; refused if untrusted."
    )]
    async fn restart_process(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::RestartProcess {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => Err(app_error(&err)),
        }
    }

    #[tool(
        description = "Write input to a process's terminal (typed text or raw control bytes). With wait_ms, returns the rendered terminal tail after waiting, so you can see the effect. Acts only within the session's project."
    )]
    async fn send_input(
        &self,
        Parameters(SendInputArg {
            process,
            input,
            wait_ms,
        }): Parameters<SendInputArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SendInput {
            process: ProcessId::from_raw(process),
            input,
            wait_ms,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::InputSent(tail)) => structured(&serde_json::json!({ "tail": tail })),
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

/// A structured acknowledgement for a state-setting tool (register / select).
fn acked() -> Result<CallToolResult, ErrorData> {
    structured(&serde_json::json!({ "ok": true }))
}

/// Maps a client error to an MCP tool error the agent can read.
fn app_error(err: &ClientError) -> ErrorData {
    ErrorData::internal_error(err.to_string(), None)
}

/// The app returned a response of the wrong shape — a protocol mismatch, not a user error.
fn unexpected() -> ErrorData {
    ErrorData::internal_error("the app returned an unexpected response".to_string(), None)
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
