//! The HTTP API's mutation routes and their handlers.
//!
//! Each handler maps to **one** core command — the same `Facade`/`Supervisor` method the
//! desktop UI and the MCP server drive — so an action like "restart" is implemented once in
//! the core and never per adapter. The token and `Host` guards apply to the whole router
//! (see [`crate::routes::router`]), so these routes need no gate of their own.
//!
//! The two bulk-start scopes are deliberate (see [`soloist_core::Supervisor`]): `start-auto`
//! starts only `auto_start` commands (the dashboard's launch-the-stack action), while
//! `start-all` starts every trusted command.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, post};
use axum::{Json, Router};

use soloist_core::{
    CoordinationError, LaunchAgentError, ProcessId, ProjectId, ReloadError, RemoveProjectError,
    SupervisorError, TodoId,
};
use soloist_ipc::http::{
    SpawnRequest, SpawnResponse, TransferScratchpadRequest, TransferTodoRequest,
};

use crate::state::ApiState;

/// The mutation sub-router. The whole-router token and `Host` guards cover these routes, so
/// they carry no gate of their own; they are merged with the read routes in
/// [`crate::routes::router`].
pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/processes/{id}/start", post(start))
        .route("/processes/{id}/stop", post(stop))
        .route("/processes/{id}/restart", post(restart))
        .route("/projects/{id}/start-auto", post(start_auto))
        .route("/projects/{id}/start-all", post(start_all))
        .route("/projects/{id}/stop-all", post(stop_all))
        .route("/projects/{id}/restart-running", post(restart_running))
        .route("/projects/{id}/restart-all", post(restart_all))
        .route("/projects/{id}/reload", post(reload))
        .route("/projects/{id}", delete(remove_project))
        .route("/projects/{id}/spawn-agent", post(spawn_agent))
        .route("/projects/{id}/transfer-todo", post(transfer_todo))
        .route(
            "/projects/{id}/transfer-scratchpad",
            post(transfer_scratchpad),
        )
        .route("/focus", post(focus))
}

/// Maps a supervisor error to the status the adapter returns: an unknown process is `404`,
/// an untrusted command is `403` (the trust gate, enforced in the core), and a durable-store
/// failure is `500`.
fn status_for(err: &SupervisorError) -> StatusCode {
    match err {
        SupervisorError::NotFound(_) => StatusCode::NOT_FOUND,
        SupervisorError::Untrusted => StatusCode::FORBIDDEN,
        SupervisorError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
        // Resume is a local UI affordance, not an HTTP mutation, so this cannot arise here;
        // mapped for exhaustiveness as a `404`, like an action on an unresumable target.
        SupervisorError::NotResumable(_) => StatusCode::NOT_FOUND,
    }
}

/// Collapses a `Result<_, SupervisorError>` to a status: `200` on success, else the mapped
/// error — the shared shape behind every trust-gated mutation.
fn outcome(result: Result<impl Sized, SupervisorError>) -> StatusCode {
    match result {
        Ok(_) => StatusCode::OK,
        Err(err) => status_for(&err),
    }
}

/// `POST /processes/:id/start` — starts one process; trust-gated in the core.
async fn start(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .blocking(move |f| f.supervisor().start(ProcessId::from_raw(id)))
            .await,
    )
}

/// `POST /processes/:id/stop` — requests a graceful stop. Idempotent: stopping a process
/// that is not live is a no-op success.
async fn stop(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    state
        .blocking(move |f| f.supervisor().stop(ProcessId::from_raw(id)))
        .await;
    StatusCode::OK
}

/// `POST /processes/:id/restart` — restarts one process; trust-gated in the core.
async fn restart(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .blocking(move |f| f.supervisor().restart(ProcessId::from_raw(id)))
            .await,
    )
}

/// `POST /projects/:id/start-auto` — starts the trusted `auto_start` commands only.
async fn start_auto(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .blocking(move |f| f.supervisor().start_all(ProjectId::from_raw(id)))
            .await,
    )
}

/// `POST /projects/:id/start-all` — starts every trusted command, regardless of `auto_start`.
async fn start_all(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .blocking(move |f| f.supervisor().start_all_commands(ProjectId::from_raw(id)))
            .await,
    )
}

/// `POST /projects/:id/stop-all` — stops every live process in the project.
async fn stop_all(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    state
        .blocking(move |f| f.supervisor().stop_all(ProjectId::from_raw(id)))
        .await;
    StatusCode::OK
}

/// `POST /projects/:id/restart-running` — restarts only the currently-running processes.
async fn restart_running(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .blocking(move |f| f.supervisor().restart_running(ProjectId::from_raw(id)))
            .await,
    )
}

/// `POST /projects/:id/restart-all` — brings the trusted command set up fresh (running ones
/// cycle, resting ones start).
async fn restart_all(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .blocking(move |f| f.supervisor().restart_all_commands(ProjectId::from_raw(id)))
            .await,
    )
}

/// `POST /projects/:id/reload` — re-reads the project's `solo.yml` and reconciles the registered
/// command set to it (adds new resting, drops removed-and-resting, updates changed specs in place,
/// applies renames), never killing running work. Routes to the one core reconcile the UI and MCP
/// can share. A byte-identical file is a no-op success; an unknown project is a `404`. The read is
/// small and bounded (the `solo.yml` cap), like the trust-store reads the other mutations make.
async fn reload(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    match state
        .blocking(move |f| f.reload_project(ProjectId::from_raw(id)))
        .await
    {
        Ok(_) => StatusCode::OK,
        Err(err) => reload_status(&err),
    }
}

/// Maps a reload failure to the status the adapter returns: an unknown project is `404`, and a
/// config re-read or durable-store failure is `500`.
fn reload_status(err: &ReloadError) -> StatusCode {
    match err {
        ReloadError::UnknownProject => StatusCode::NOT_FOUND,
        ReloadError::Sync(_) | ReloadError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// `DELETE /projects/:id` — removes the project from Soloist: closes its processes (each live
/// group stopped and reaped before anything is forgotten), deletes its durable record (the
/// store cascades to its project-scoped state), and announces the removal. Routes to the one
/// core removal the desktop confirm dialog drives; files on disk are never touched. An
/// unknown project is a `404`.
async fn remove_project(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    match state.facade().remove_project(ProjectId::from_raw(id)).await {
        Ok(()) => StatusCode::OK,
        Err(RemoveProjectError::UnknownProject) => StatusCode::NOT_FOUND,
        Err(RemoveProjectError::Store(_)) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// `POST /focus` — raises the desktop window so a launcher can bring Soloist to the front.
async fn focus(State(state): State<ApiState>) -> StatusCode {
    state.focus();
    StatusCode::OK
}

/// `POST /projects/:id/spawn-agent` — launches a **known** configured agent tool as a worker in
/// the project and starts it, returning the new process's id. Routes to the same
/// [`Facade::launch_agent`] the desktop launch picker drives — the local user's authority on the
/// loopback socket (an ungated `Agent`-kind process), not the session-scoped MCP `spawn_agent`,
/// which stays MCP-only. An unknown tool or project is a `404`.
async fn spawn_agent(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Json(body): Json<SpawnRequest>,
) -> Result<Json<SpawnResponse>, StatusCode> {
    // Bound a runaway caller before doing any work: past the per-launch cap, refuse with 429.
    if !state.allow_spawn() {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    match state
        .blocking(move |f| f.launch_agent(ProjectId::from_raw(id), &body.tool, body.args))
        .await
    {
        Ok(process) => Ok(Json(SpawnResponse { id: process.get() })),
        Err(err) => Err(launch_status(&err)),
    }
}

/// Maps an agent-launch failure to the status the adapter returns: an unknown tool or project is
/// `404`, and a durable-store or supervisor failure is `500`.
fn launch_status(err: &LaunchAgentError) -> StatusCode {
    match err {
        LaunchAgentError::UnknownTool | LaunchAgentError::UnknownProject => StatusCode::NOT_FOUND,
        LaunchAgentError::Store(_) | LaunchAgentError::Supervisor(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// `POST /projects/:id/transfer-todo` — moves a todo from the path (source) project to another,
/// via the same [`Facade::todo_transfer_in`] a local surface drives (the local user's authority on
/// the loopback socket, which addresses both projects by explicit id). Keeps the todo's document,
/// comments, tags, and id; clears its blockers and lock (they reference the source project). An
/// unknown todo or target project is a `404`.
async fn transfer_todo(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Json(body): Json<TransferTodoRequest>,
) -> StatusCode {
    transfer_status(
        state
            .blocking(move |f| {
                f.todo_transfer_in(
                    ProjectId::from_raw(id),
                    ProjectId::from_raw(body.to_project),
                    TodoId::from_raw(body.todo),
                )
            })
            .await,
    )
}

/// `POST /projects/:id/transfer-scratchpad` — moves a scratchpad by name from the path (source)
/// project to another, via [`Facade::scratchpad_transfer_in`]. Keeps its document, revision, tags,
/// and id. An unknown scratchpad or target project is a `404`; a name already used in the target is
/// a `409`.
async fn transfer_scratchpad(
    State(state): State<ApiState>,
    Path(id): Path<u64>,
    Json(body): Json<TransferScratchpadRequest>,
) -> StatusCode {
    transfer_status(
        state
            .blocking(move |f| {
                f.scratchpad_transfer_in(
                    ProjectId::from_raw(id),
                    &body.name,
                    ProjectId::from_raw(body.to_project),
                )
            })
            .await,
    )
}

/// Collapses a transfer outcome to a status: `200` on success; an unknown todo, scratchpad, or
/// target project is `404`; a name already used in the target is `409`; a durable failure is `500`.
/// Only the outcomes the two local `*_transfer_in` paths can produce are named; the scope/binding
/// variants of [`CoordinationError`] cannot arise on this path (it takes explicit project ids, not
/// a session), so they fall through to `500`.
fn transfer_status(result: Result<impl Sized, CoordinationError>) -> StatusCode {
    match result {
        Ok(_) => StatusCode::OK,
        Err(
            CoordinationError::UnknownTodo
            | CoordinationError::UnknownScratchpad
            | CoordinationError::UnknownProject,
        ) => StatusCode::NOT_FOUND,
        Err(CoordinationError::ScratchpadNameTaken) => StatusCode::CONFLICT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
