//! The local IPC server: the Unix-socket front the MCP server (`soloist-mcp`) connects to.
//!
//! This is the app-side half of the [`soloist_ipc`] transport — a driving adapter compiled
//! in only under the `mcp` feature, so turning the feature off drops it (and its dependency)
//! and the app still builds and runs. Each connection is one identity session; every request
//! routes through [`handle_request`] to exactly one [`Facade`] method, so MCP, the UI, and
//! the HTTP API share one behaviour and the read model projects back. The server holds no
//! business state.

use std::time::Duration;

use crate::peer_cred;
use soloist_core::{Facade, ProjectId, SessionId, WaitForPortError};
use soloist_ipc::{
    ensure_socket_path, read_frame, write_frame, IpcError, IpcRequest, IpcResponse, IpcResult,
    PortWaitOutcome, ProjectStatus, ProjectSummary,
};
use tauri::{AppHandle, Manager};
use tokio::net::{UnixListener, UnixStream};

/// The port-readiness wait when the caller names no timeout.
const DEFAULT_PORT_WAIT: Duration = Duration::from_secs(10);
/// The longest a `wait_for_bound_port` blocks, regardless of the requested timeout. Kept
/// well under the IPC client's per-request timeout so the wait resolves as a structured
/// "not bound yet" rather than a transport timeout, and a remote caller cannot tie up the
/// connection with a huge value.
const MAX_PORT_WAIT: Duration = Duration::from_secs(25);

/// Binds the IPC socket and serves connections until the app shuts down. Degrades to a
/// logged no-op if the socket cannot be resolved or bound, so a packaging or permissions
/// problem disables MCP rather than taking down the app (graceful degradation).
pub async fn serve(app: AppHandle) {
    // Resolves the socket path and creates its owner-only data directory in one step — the
    // single resolution the store shares, so the socket and database keep one private home.
    let path = match ensure_socket_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("soloist: MCP IPC disabled (cannot prepare the socket directory: {err})");
            return;
        }
    };
    // A leftover socket from a previous run would make bind fail; the path is ours to clear.
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!(
                "soloist: MCP IPC disabled (cannot bind {}: {err})",
                path.display()
            );
            return;
        }
    };
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                tauri::async_runtime::spawn(handle_connection(app.clone(), stream));
            }
            Err(err) => {
                eprintln!("soloist: MCP IPC stopped accepting connections: {err}");
                return;
            }
        }
    }
}

/// Serves one client connection: reads the connecting peer's process group, opens an identity
/// session bound to it, answers framed requests until the peer disconnects, then closes the
/// session so its scope and binding are forgotten. The peer group is what authenticates a
/// session's project scope — the core matches it to the managed process the caller runs in —
/// so a client cannot bind to or act on a sibling project it does not run in. A connection
/// whose peer credentials cannot be read at all is dropped (fail closed).
async fn handle_connection(app: AppHandle, mut stream: UnixStream) {
    let peer_pgid = match peer_cred::peer_pgid(&stream) {
        Ok(peer_pgid) => peer_pgid,
        Err(err) => {
            eprintln!(
                "soloist: MCP IPC dropped a connection (cannot read peer credentials: {err})"
            );
            return;
        }
    };
    let session = app.state::<Facade>().open_session(peer_pgid);
    loop {
        let request: IpcRequest = match read_frame(&mut stream).await {
            Ok(Some(request)) => request,
            Ok(None) => break, // the peer closed the connection
            Err(err) => {
                eprintln!("soloist: MCP IPC read error: {err}");
                break;
            }
        };
        let reply = handle_request(app.state::<Facade>().inner(), session, request).await;
        if let Err(err) = write_frame(&mut stream, &reply).await {
            eprintln!("soloist: MCP IPC write error: {err}");
            break;
        }
    }
    app.state::<Facade>().close_session(session);
}

/// Routes one request to the single matching [`Facade`] method and projects the result
/// back. The only place the IPC wire meets the core — and it adds no domain logic of its
/// own (identity, scope, and the trust gate all live in the core). Async because some
/// behaviours (e.g. `send_input` with a wait) await the core.
async fn handle_request(facade: &Facade, session: SessionId, request: IpcRequest) -> IpcResult {
    match request {
        IpcRequest::Whoami => Ok(IpcResponse::Whoami(facade.whoami(session))),
        IpcRequest::BindSessionProcess { process } => facade
            .bind_session_process(session, process)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::RegisterAgent { label } => {
            facade.register_agent(session, label);
            Ok(IpcResponse::Acked)
        }
        IpcRequest::SelectProject { project } => facade
            .select_project(session, project)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::ListProjects => Ok(IpcResponse::Projects(project_summaries(facade)?)),
        IpcRequest::GetProjectStatus { project } => project_status(facade, session, project),
        IpcRequest::ListProcesses => Ok(IpcResponse::Processes(facade.snapshot())),
        IpcRequest::GetProcessStatus { process } => facade
            .process_view(process)
            .map(IpcResponse::Process)
            .ok_or(IpcError::UnknownProcess),
        IpcRequest::StartProcess { process } => facade
            .start_process(session, process)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::StopProcess { process } => facade
            .stop_process(session, process)
            .map(IpcResponse::Stopped)
            .map_err(IpcError::from),
        IpcRequest::RestartProcess { process } => facade
            .restart_process(session, process)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::SendInput {
            process,
            input,
            wait_ms,
        } => facade
            .send_input(
                session,
                process,
                input.into_bytes(),
                wait_ms.map(Duration::from_millis),
            )
            .await
            .map(IpcResponse::InputSent)
            .map_err(IpcError::from),
        IpcRequest::SpawnAgent { tool, extra_args } => facade
            .spawn_agent(session, &tool, extra_args)
            .map(IpcResponse::Spawned)
            .map_err(IpcError::from),
        IpcRequest::ListAgentTools => facade
            .agents()
            .list_tools()
            .map(IpcResponse::AgentTools)
            .map_err(|err| IpcError::Internal(err.to_string())),
        IpcRequest::StartAllCommands => facade
            .start_all_commands(session)
            .map(IpcResponse::BulkStarted)
            .map_err(IpcError::from),
        IpcRequest::StopAllCommands => facade
            .stop_all_commands(session)
            .map(IpcResponse::BulkStopped)
            .map_err(IpcError::from),
        IpcRequest::RestartAllCommands => facade
            .restart_all_commands(session)
            .map(|()| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::GetProcessOutput { process, lines } => facade
            .process_output(process, lines)
            .map(IpcResponse::Lines)
            .ok_or(IpcError::UnknownProcess),
        IpcRequest::GetProcessRawOutput { process } => facade
            .process_raw_output(process)
            .map(|bytes| IpcResponse::RawOutput(String::from_utf8_lossy(&bytes).into_owned()))
            .ok_or(IpcError::UnknownProcess),
        IpcRequest::SearchOutput {
            process,
            query,
            limit,
        } => facade
            .search_output(process, &query, limit)
            .map(IpcResponse::Lines)
            .ok_or(IpcError::UnknownProcess),
        IpcRequest::SearchRawOutput {
            process,
            query,
            limit,
        } => facade
            .search_raw_output(process, &query, limit)
            .map(IpcResponse::Lines)
            .ok_or(IpcError::UnknownProcess),
        IpcRequest::ClearOutput { process } => facade
            .clear_output(session, process)
            .map(|_| IpcResponse::Acked)
            .map_err(IpcError::from),
        IpcRequest::FlushTerminalPerf { process } => facade
            .flush_terminal_perf(process)
            .then_some(IpcResponse::Acked)
            .ok_or(IpcError::UnknownProcess),
        IpcRequest::GetProcessPorts { process } => facade
            .process_ports(process)
            .map(IpcResponse::Ports)
            .ok_or(IpcError::UnknownProcess),
        IpcRequest::ServicesList => facade
            .services_list(session)
            .map(IpcResponse::Processes)
            .map_err(IpcError::from),
        IpcRequest::WaitForBoundPort {
            process,
            port,
            timeout_ms,
        } => {
            let timeout = timeout_ms
                .map_or(DEFAULT_PORT_WAIT, Duration::from_millis)
                .min(MAX_PORT_WAIT);
            let outcome = match facade.wait_for_port(process, port, timeout).await {
                Ok(()) => PortWaitOutcome::Bound,
                Err(WaitForPortError::Timeout) => PortWaitOutcome::TimedOut,
                Err(WaitForPortError::NotRunning) => PortWaitOutcome::NotRunning,
            };
            Ok(IpcResponse::PortWait(outcome))
        }
    }
}

/// Every loaded project as a lean, agent-facing summary.
fn project_summaries(facade: &Facade) -> Result<Vec<ProjectSummary>, IpcError> {
    Ok(facade
        .projects_snapshot()
        .map_err(|err| IpcError::Internal(err.to_string()))?
        .iter()
        .map(ProjectSummary::from_view)
        .collect())
}

/// One project (explicit, or the session's effective scope) with its current processes.
fn project_status(facade: &Facade, session: SessionId, project: Option<ProjectId>) -> IpcResult {
    let target = match project {
        Some(project) => project,
        None => facade
            .whoami(session)
            .effective_project
            .ok_or(IpcError::NoProjectScope)?,
    };
    let view = facade
        .projects_snapshot()
        .map_err(|err| IpcError::Internal(err.to_string()))?
        .into_iter()
        .find(|view| view.id == target)
        .ok_or(IpcError::UnknownProject)?;
    let processes = facade
        .snapshot()
        .into_iter()
        .filter(|view| view.project == target)
        .collect();
    Ok(IpcResponse::ProjectStatus(ProjectStatus {
        project: ProjectSummary::from_view(&view),
        processes,
    }))
}

#[cfg(test)]
#[path = "ipc_server_tests.rs"]
mod tests;
