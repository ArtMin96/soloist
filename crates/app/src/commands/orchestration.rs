//! The orchestration read-model command: the one read the orchestration tree renders.
//!
//! A thin wrapper that routes straight to the one [`Facade`] query — no logic here. Like
//! [`proc_list`](super::proc_list) it is a **local** read: the trusted local UI hands the
//! `project` it already has access to. It is registered only for the local Tauri surface; an
//! MCP or HTTP exposure would have to derive `project` from the caller's bound, identity-checked
//! scope instead (see [`Facade::orchestration_snapshot`]).

use std::sync::Arc;

use soloist_core::{Facade, LineageEdge, OrchestrationSnapshot, ProjectId};
use tauri::State;

/// The orchestration read-model for `project`: its agent lineage tree plus the coordination
/// state agents share (todos, timers, leases, scratchpads, key-value). The snapshot half of
/// snapshot-then-deltas for the orchestration tree; a coordination or process-lifecycle
/// domain event prompts the UI to re-read this.
#[tauri::command]
pub async fn orchestration_snapshot(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<OrchestrationSnapshot, String> {
    facade
        .orchestration_snapshot(project)
        .map_err(|err| err.to_string())
}

/// Every live spawn-lineage edge across all projects — the sidebar joins these onto its process
/// list to nest workers under their leads, re-reading on process lifecycle events.
#[tauri::command]
pub async fn lineage_edges(facade: State<'_, Arc<Facade>>) -> Result<Vec<LineageEdge>, String> {
    Ok(facade.lineage_edges())
}
