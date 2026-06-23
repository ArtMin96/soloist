//! Output tools: reading, searching, and clearing a process's terminal output, and listing
//! its detected ports.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::ProcessId;
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{OutputArg, ProcessArg, SearchArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{acked, app_error, structured, unexpected};

#[tool_router(router = output_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Read a process's recent rendered terminal output (escape sequences applied) as lines, most recent last. Use `lines` to bound how many; omit for the server default."
    )]
    pub(crate) async fn get_process_output(
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
    pub(crate) async fn get_process_raw_output(
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
    pub(crate) async fn search_output(
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
    pub(crate) async fn search_raw_output(
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
    pub(crate) async fn clear_output(
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
    pub(crate) async fn flush_terminal_perf(
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
    pub(crate) async fn get_process_ports(
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
