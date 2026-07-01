//! The HTTP API's mutation routes and their handlers.
//!
//! Each handler maps to **one** core command — the same `Facade`/`Supervisor` method the
//! desktop UI and the MCP server drive — so an action like "restart" is implemented once in
//! the core and never per adapter. Every route here sits behind the local-auth gate; the
//! read routes stay open on loopback.
//!
//! The two bulk-start scopes are deliberate (see [`soloist_core::Supervisor`]): `start-auto`
//! starts only `auto_start` commands (the dashboard's launch-the-stack action), while
//! `start-all` starts every trusted command.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::post;
use axum::{Json, Router};

use soloist_core::{LaunchAgentError, ProcessId, ProjectId, SupervisorError};
use soloist_ipc::http::{SpawnRequest, SpawnResponse};

use crate::auth::require_local_auth;
use crate::state::ApiState;

/// The mutation sub-router, with the local-auth gate applied to every route on it. Using
/// `route_layer` confines the gate to these routes — the read routes merged alongside stay
/// open on loopback.
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
        .route("/projects/{id}/spawn-agent", post(spawn_agent))
        .route("/focus", post(focus))
        .route_layer(middleware::from_fn(require_local_auth))
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
    outcome(state.facade().supervisor().start(ProcessId::from_raw(id)))
}

/// `POST /processes/:id/stop` — requests a graceful stop. Idempotent: stopping a process
/// that is not live is a no-op success.
async fn stop(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    state.facade().supervisor().stop(ProcessId::from_raw(id));
    StatusCode::OK
}

/// `POST /processes/:id/restart` — restarts one process; trust-gated in the core.
async fn restart(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(state.facade().supervisor().restart(ProcessId::from_raw(id)))
}

/// `POST /projects/:id/start-auto` — starts the trusted `auto_start` commands only.
async fn start_auto(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .facade()
            .supervisor()
            .start_all(ProjectId::from_raw(id)),
    )
}

/// `POST /projects/:id/start-all` — starts every trusted command, regardless of `auto_start`.
async fn start_all(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .facade()
            .supervisor()
            .start_all_commands(ProjectId::from_raw(id)),
    )
}

/// `POST /projects/:id/stop-all` — stops every live process in the project.
async fn stop_all(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    state
        .facade()
        .supervisor()
        .stop_all(ProjectId::from_raw(id));
    StatusCode::OK
}

/// `POST /projects/:id/restart-running` — restarts only the currently-running processes.
async fn restart_running(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .facade()
            .supervisor()
            .restart_running(ProjectId::from_raw(id)),
    )
}

/// `POST /projects/:id/restart-all` — brings the trusted command set up fresh (running ones
/// cycle, resting ones start).
async fn restart_all(State(state): State<ApiState>, Path(id): Path<u64>) -> StatusCode {
    outcome(
        state
            .facade()
            .supervisor()
            .restart_all_commands(ProjectId::from_raw(id)),
    )
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
    match state
        .facade()
        .launch_agent(ProjectId::from_raw(id), &body.tool, body.args)
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
