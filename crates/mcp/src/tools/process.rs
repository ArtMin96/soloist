//! Process tools: inspecting and controlling one process by its id. Action tools act only
//! within the session's effective project and honour the trust gate — both enforced in the
//! core, not here.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::ProcessId;
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{ProcessArg, RenameArg, SendInputArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{acked, app_error, structured, unexpected};

#[tool_router(router = process_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(description = "List every process Soloist supervises across all projects.")]
    pub(crate) async fn list_processes(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ListProcesses).await {
            Ok(IpcResponse::Processes(processes)) => structured(&processes),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(description = "Get one process's current status by its id.")]
    pub(crate) async fn get_process_status(
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
    pub(crate) async fn start_process(
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
    pub(crate) async fn stop_process(
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
    pub(crate) async fn restart_process(
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
        description = "Rename one process's display label by its id. Display-only: it does not affect trust or what the process runs. Acts only within the session's project."
    )]
    pub(crate) async fn rename_process(
        &self,
        Parameters(RenameArg { process, label }): Parameters<RenameArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::RenameProcess {
            process: ProcessId::from_raw(process),
            label,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::Acked) => acked(),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Stop one process and remove it from Soloist entirely by its id (its terminal scrollback is discarded). Use stop_process to stop without removing. Acts only within the session's project."
    )]
    pub(crate) async fn close_process(
        &self,
        Parameters(ProcessArg { process }): Parameters<ProcessArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::CloseProcess {
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
    pub(crate) async fn send_input(
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
}
