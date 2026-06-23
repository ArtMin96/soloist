//! The rmcp server handler: each tool a thin call to one IPC request — read and action
//! tools alike.
//!
//! Tool *names* mirror Solo for interop, but the parameter schemas are clean-room — derived
//! from the argument structs here. No domain logic lives in a tool: each forwards to the app,
//! which resolves identity, scope, and the trust gate in the core, and the result is returned
//! as structured content.

use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorData};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use serde::Serialize;
use soloist_core::{ProcessId, ProjectId};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{
    OutputArg, ProcessArg, ProjectArg, RegisterAgentArg, SearchArg, SelectProjectArg, SendInputArg,
    SpawnAgentArg,
};
use crate::client::{AppClient, ClientError};

/// The Soloist MCP server: a stateless front over the app, holding one client connection.
#[derive(Clone)]
pub struct SoloistMcp {
    client: Arc<AppClient>,
    tool_router: ToolRouter<Self>,
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
            Err(err) => app_error(&err),
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
            Err(err) => app_error(&err),
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
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "List every project Soloist has open.")]
    async fn list_projects(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListProjects).await {
            Ok(IpcResponse::Projects(projects)) => structured(&projects),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
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
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "List every process Soloist supervises across all projects.")]
    async fn list_processes(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListProcesses).await {
            Ok(IpcResponse::Processes(processes)) => structured(&processes),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
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
            Err(err) => app_error(&err),
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
            Err(err) => app_error(&err),
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
            Err(err) => app_error(&err),
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
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Write input to a process's terminal as UTF-8 text, including control characters (a trailing carriage return submits a line; 0x03 is Ctrl-C). With wait_ms, returns the rendered terminal tail after waiting, so you can see the effect. Acts only within the session's project."
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
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Spawn a configured agent tool as a worker in this session's project and start it. Use `list_agent_tools` for the available names. Returns the new process id."
    )]
    async fn spawn_agent(
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
    async fn list_agent_tools(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListAgentTools).await {
            Ok(IpcResponse::AgentTools(tools)) => structured(&tools),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Start every trusted command in this session's project (whatever its auto-start setting). Returns the ids that started and any skipped as untrusted. Agents and terminals are untouched."
    )]
    async fn start_all_commands(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::StartAllCommands).await {
            Ok(IpcResponse::BulkStarted(summary)) => structured(&summary),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Gracefully stop every running command in this session's project. Leaves agents and terminals running. Returns how many commands were stopped."
    )]
    async fn stop_all_commands(&self) -> Result<CallToolResult, ErrorData> {
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
    async fn restart_all_commands(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::RestartAllCommands).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read a process's recent rendered terminal output (escape sequences applied) as lines, most recent last. Use `lines` to bound how many; omit for the server default."
    )]
    async fn get_process_output(
        &self,
        Parameters(OutputArg { process, lines }): Parameters<OutputArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GetProcessOutput {
            process: ProcessId::from_raw(process),
            lines,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Lines(lines)) => structured(&serde_json::json!({ "output": lines })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read a process's raw terminal output including control sequences, decoded as UTF-8. For when you need the bytes a terminal emulator would see; use `get_process_output` for plain text."
    )]
    async fn get_process_raw_output(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GetProcessRawOutput {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::RawOutput(raw)) => structured(&serde_json::json!({ "output": raw })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Find rendered output lines of a process containing a substring (case-sensitive). Returns the matching lines, in order, bounded by `limit`."
    )]
    async fn search_output(
        &self,
        Parameters(SearchArg {
            process,
            query,
            limit,
        }): Parameters<SearchArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SearchOutput {
            process: ProcessId::from_raw(process),
            query,
            limit,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Lines(matches)) => {
                structured(&serde_json::json!({ "matches": matches }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Find raw output lines of a process containing a substring (case-sensitive), control sequences included. Returns the matching lines, in order, bounded by `limit`."
    )]
    async fn search_raw_output(
        &self,
        Parameters(SearchArg {
            process,
            query,
            limit,
        }): Parameters<SearchArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::SearchRawOutput {
            process: ProcessId::from_raw(process),
            query,
            limit,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Lines(matches)) => {
                structured(&serde_json::json!({ "matches": matches }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Clear a process's output buffers (the rendered and raw scrollback Soloist keeps), without stopping the process or touching its terminal. Acts only within the session's project."
    )]
    async fn clear_output(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::ClearOutput {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Flush a process's terminal output for performance. A no-op in Soloist — the output you read is always current — kept for tool compatibility."
    )]
    async fn flush_terminal_perf(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::FlushTerminalPerf {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "List the localhost ports a process is currently listening on (its detected dev-server ports)."
    )]
    async fn get_process_ports(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::GetProcessPorts {
            process: ProcessId::from_raw(process),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Ports(ports)) => structured(&serde_json::json!({ "ports": ports })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
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

/// Maps a failed request to the agent-visible failure, per the MCP error model. A
/// request-caused refusal (untrusted, out of scope, no project selected, unknown
/// process/project/tool) becomes a tool-execution error (`isError: true`) — actionable
/// feedback the model can self-correct on. A transport or server failure (app down, timeout,
/// internal) stays a protocol error, which the model is less likely to recover from.
fn app_error(err: &ClientError) -> Result<CallToolResult, ErrorData> {
    match err {
        ClientError::App(app) if app.is_request_error() => {
            Ok(CallToolResult::error(vec![Content::text(app.to_string())]))
        }
        _ => Err(ErrorData::internal_error(err.to_string(), None)),
    }
}

/// The app returned a response of the wrong shape — a protocol mismatch, not a user error.
fn unexpected() -> ErrorData {
    ErrorData::internal_error("the app returned an unexpected response".to_string(), None)
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
