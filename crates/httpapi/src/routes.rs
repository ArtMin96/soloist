//! The HTTP API's read routes and their handlers. Each handler maps to one [`Facade`]
//! read; the response bodies reuse the core read-model types, so the wire shape stays
//! single-source with the UI and MCP.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use soloist_core::{ProcStatus, ProcessId, ProcessView, ProjectView};

use crate::cors::localhost_cors;
use crate::state::ApiState;

/// Builds the router: the read routes, with the localhost CORS layer applied to all of
/// them. Mutation routes and their auth gate land in their own slice.
pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/processes", get(processes))
        .route("/processes/{id}/ports", get(process_ports))
        .route("/projects", get(projects))
        .layer(localhost_cors())
        .with_state(state)
}

/// `GET /health` — liveness plus the running version, so a client can confirm it reached
/// Soloist and which build.
async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize)]
struct Health {
    ok: bool,
    version: &'static str,
}

/// `GET /status` — a small cross-project summary: how many projects are open and a tally
/// of processes, for a shell to glance at without reading every row.
async fn status(State(state): State<ApiState>) -> Result<Json<Status>, StatusCode> {
    let processes = state.facade().snapshot();
    let running = processes
        .iter()
        .filter(|process| process.status == ProcStatus::Running)
        .count();
    let projects = state
        .facade()
        .projects_snapshot()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .len();
    Ok(Json(Status {
        projects,
        processes: processes.len(),
        running,
    }))
}

#[derive(Serialize)]
struct Status {
    projects: usize,
    processes: usize,
    running: usize,
}

/// `GET /processes` — the live process read model as JSON.
async fn processes(State(state): State<ApiState>) -> Json<Vec<ProcessView>> {
    Json(state.facade().snapshot())
}

/// `GET /processes/:id/ports` — the TCP ports a process is currently listening on. An
/// unknown id has no row and so reads as an empty list.
async fn process_ports(State(state): State<ApiState>, Path(id): Path<u64>) -> Json<Vec<u16>> {
    let ports = state
        .facade()
        .process_view(ProcessId::from_raw(id))
        .map(|view| view.ports)
        .unwrap_or_default();
    Json(ports)
}

/// `GET /projects` — every opened project's display identity.
async fn projects(State(state): State<ApiState>) -> Result<Json<Vec<ProjectView>>, StatusCode> {
    state
        .facade()
        .projects_snapshot()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
