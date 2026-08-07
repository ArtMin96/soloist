//! The version-control commands about which branch is checked out and where the working tree's
//! changes are kept: thin wrappers that route to the one core method.
//!
//! No logic here — not the trust gate, not the guard on what may be handed over as a branch name,
//! and not the decision about what version control would refuse. All of them are the core's or the
//! tool's own, so nothing here has to remember them.

use std::sync::Arc;

use soloist_core::{Branches, Facade, ProjectId};
use tauri::State;

/// The branches this project could switch to, most recently committed to first and bounded by the
/// core's own page size, plus whether anything is stashed. `null` for a project that is not a
/// repository, the same ordinary state a status read reports it as.
///
/// Reading a repository runs an external tool, so it goes to the blocking pool.
#[tauri::command]
pub async fn git_branches(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<Option<Branches>, String> {
    facade
        .blocking(move |f| f.git_branches(project))
        .await
        .map_err(|err| err.to_string())
}

/// Starts a branch at what is checked out and switches to it.
#[tauri::command]
pub async fn git_create_branch(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    name: String,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_create_branch(project, &name))
        .await
        .map_err(|err| err.to_string())
}

/// Checks out an existing branch. A switch that would overwrite uncommitted work comes back as
/// version control's own account of what is in the way; nothing is stashed or forced past it.
#[tauri::command]
pub async fn git_switch_branch(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    name: String,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_switch_branch(project, &name))
        .await
        .map_err(|err| err.to_string())
}

/// Removes a branch. Destructive, so a surface confirms it first — and bounded, because a branch
/// holding commits nothing else holds is refused and stays refused.
#[tauri::command]
pub async fn git_delete_branch(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    name: String,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_delete_branch(project, &name))
        .await
        .map_err(|err| err.to_string())
}

/// Sets what the working tree holds aside, leaving it as the last commit left it. A file version
/// control does not track stays where it is.
#[tauri::command]
pub async fn git_stash(facade: State<'_, Arc<Facade>>, project: ProjectId) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_stash(project))
        .await
        .map_err(|err| err.to_string())
}

/// Puts the most recently stashed changes back. A collision with what the working tree holds now
/// comes back as version control's own account of it, because a conflict was left to resolve rather
/// than the change being applied.
#[tauri::command]
pub async fn git_pop_stash(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_pop_stash(project))
        .await
        .map_err(|err| err.to_string())
}
