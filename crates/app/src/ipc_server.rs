//! The local IPC server: the Unix-socket front the MCP server (`soloist-mcp`) connects to.
//!
//! This is the app-side half of the [`soloist_ipc`] transport — a driving adapter compiled
//! in only under the `mcp` feature, so turning the feature off drops it (and its dependency)
//! and the app still builds and runs. Each connection is one identity session; every request
//! routes through [`handle_request`] to exactly one [`Facade`] method, so MCP, the UI, and
//! the HTTP API share one behaviour and the read model projects back. The server holds no
//! business state.

use soloist_core::{Facade, IdentityError, ProjectId, SessionId};
use soloist_ipc::{
    read_frame, socket_path, write_frame, IpcError, IpcRequest, IpcResponse, IpcResult,
    ProjectStatus, ProjectSummary,
};
use tauri::{AppHandle, Manager};
use tokio::net::{UnixListener, UnixStream};

/// Binds the IPC socket and serves connections until the app shuts down. Degrades to a
/// logged no-op if the socket cannot be resolved or bound, so a packaging or permissions
/// problem disables MCP rather than taking down the app (graceful degradation).
pub async fn serve(app: AppHandle) {
    let path = match socket_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("soloist: MCP IPC disabled (cannot resolve socket path: {err})");
            return;
        }
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
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

/// Serves one client connection: opens an identity session, answers framed requests until
/// the peer disconnects, then closes the session so its scope and binding are forgotten.
async fn handle_connection(app: AppHandle, mut stream: UnixStream) {
    let session = app.state::<Facade>().open_session();
    loop {
        let request: IpcRequest = match read_frame(&mut stream).await {
            Ok(Some(request)) => request,
            Ok(None) => break, // the peer closed the connection
            Err(err) => {
                eprintln!("soloist: MCP IPC read error: {err}");
                break;
            }
        };
        let reply = handle_request(app.state::<Facade>().inner(), session, request);
        if let Err(err) = write_frame(&mut stream, &reply).await {
            eprintln!("soloist: MCP IPC write error: {err}");
            break;
        }
    }
    app.state::<Facade>().close_session(session);
}

/// Routes one request to the single matching [`Facade`] method and projects the result
/// back. The only place the IPC wire meets the core — and it adds no domain logic of its
/// own (identity, scope, and the trust gate all live in the core).
fn handle_request(facade: &Facade, session: SessionId, request: IpcRequest) -> IpcResult {
    match request {
        IpcRequest::Whoami => Ok(IpcResponse::Whoami(facade.whoami(session))),
        IpcRequest::BindSessionProcess { process } => facade
            .bind_session_process(session, process)
            .map(|()| IpcResponse::Acked)
            .map_err(into_ipc_error),
        IpcRequest::RegisterAgent { label } => {
            facade.register_agent(session, label);
            Ok(IpcResponse::Acked)
        }
        IpcRequest::SelectProject { project } => facade
            .select_project(session, project)
            .map(|()| IpcResponse::Acked)
            .map_err(into_ipc_error),
        IpcRequest::ListProjects => Ok(IpcResponse::Projects(project_summaries(facade)?)),
        IpcRequest::GetProjectStatus { project } => project_status(facade, session, project),
        IpcRequest::ListProcesses => Ok(IpcResponse::Processes(facade.snapshot())),
        IpcRequest::GetProcessStatus { process } => facade
            .snapshot()
            .into_iter()
            .find(|view| view.id == process)
            .map(IpcResponse::Process)
            .ok_or(IpcError::UnknownProcess),
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

/// Maps a core identity error to its wire form.
fn into_ipc_error(err: IdentityError) -> IpcError {
    match err {
        IdentityError::UnknownProcess => IpcError::UnknownProcess,
        IdentityError::UnknownProject => IpcError::UnknownProject,
        IdentityError::Store(err) => IpcError::Internal(err.to_string()),
    }
}

#[cfg(test)]
#[path = "ipc_server_tests.rs"]
mod tests;
