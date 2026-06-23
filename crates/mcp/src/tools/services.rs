//! Services tools: the session project's command processes and waiting for one to bind a
//! port.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_core::ProcessId;
use soloist_ipc::{IpcRequest, IpcResponse, PortWaitOutcome};

use crate::args::WaitForPortArg;
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = services_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "List the services (command processes) of this session's project with their status, detected ports, and readiness."
    )]
    pub(crate) async fn services_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::ServicesList).await {
            Ok(IpcResponse::Processes(services)) => {
                structured(&serde_json::json!({ "services": services }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Wait until a process is listening on a port, then return. Returns `bound: true` on success, or `bound: false` with a reason if it times out or the process is not running. Use to wait for a dev server before acting on it."
    )]
    pub(crate) async fn wait_for_bound_port(
        &self,
        Parameters(WaitForPortArg {
            process,
            port,
            timeout_ms,
        }): Parameters<WaitForPortArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::WaitForBoundPort {
            process: ProcessId::from_raw(process),
            port,
            timeout_ms,
        };
        match self.client.request(request).await {
            Ok(IpcResponse::PortWait(outcome)) => structured(&port_wait_json(outcome)),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}

/// Projects a port-wait outcome to the agent-facing JSON: `bound` plus, when it did not
/// bind, the reason the model can act on.
fn port_wait_json(outcome: PortWaitOutcome) -> serde_json::Value {
    match outcome {
        PortWaitOutcome::Bound => serde_json::json!({ "bound": true }),
        PortWaitOutcome::TimedOut => {
            serde_json::json!({ "bound": false, "reason": "timed_out" })
        }
        PortWaitOutcome::NotRunning => {
            serde_json::json!({ "bound": false, "reason": "not_running" })
        }
    }
}
