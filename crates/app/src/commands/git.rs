//! The version-control commands: thin wrappers that route to the one core read.
//!
//! No logic here. Like [`proc_list`](super::proc_list) these are **local** reads: the trusted
//! local UI hands the `project` it already has access to. A scope-limited surface would have to
//! derive it from the caller's bound, identity-checked session instead.

use std::sync::Arc;

use soloist_core::{Facade, GitStatus, ProjectFile, ProjectId};
use tauri::State;

/// A project's working-tree status — what is checked out, how it stands against its upstream,
/// and every path that differs from the last commit. `null` for a project that is not a
/// repository, which is an ordinary state rather than an error. The snapshot half of
/// snapshot-then-deltas for version control; a `GitStatusChanged` event prompts a re-read.
///
/// Reading a repository runs an external tool, so it goes to the blocking pool.
#[tauri::command]
pub async fn git_status(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<Option<GitStatus>, String> {
    facade
        .blocking(move |f| f.git_status(project))
        .await
        .map_err(|err| err.to_string())
}

/// Every path in a project's repository — tracked, untracked, and ignored, with an ignored
/// directory listed as itself rather than walked. `null` for a project that is not a
/// repository, the same ordinary state [`git_status`] reports it as.
///
/// Reading a repository runs an external tool, so it goes to the blocking pool.
#[tauri::command]
pub async fn git_files(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<Option<Vec<ProjectFile>>, String> {
    facade
        .blocking(move |f| f.git_files(project))
        .await
        .map_err(|err| err.to_string())
}
