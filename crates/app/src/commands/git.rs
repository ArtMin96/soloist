//! The version-control commands: thin wrappers that route to the one core method.
//!
//! No logic here — including the trust gate, which is spent in the core so an agent over MCP
//! and a future remote caller are refused by the same rule rather than by a check each surface
//! remembers to make. Like [`proc_list`](super::proc_list) these are **local**: the trusted
//! local UI hands the `project` it already has access to. A scope-limited surface would have to
//! derive it from the caller's bound, identity-checked session instead.

use std::sync::Arc;

use soloist_core::{
    DiffExtent, DiffTarget, Facade, FileContent, FileDiff, GitStatus, HunkRange, ProjectFile,
    ProjectId,
};
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

/// How one path differs, `target` deciding against what and `extent` how much of the answer is
/// carried. A diff longer than one capped read arrives marked truncated; asking again at
/// `full` carries the rest. `null` for a project that is not a repository and for a path that
/// does not name something inside it.
///
/// Reading a repository runs an external tool, so it goes to the blocking pool.
#[tauri::command]
pub async fn git_diff(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    path: String,
    target: DiffTarget,
    extent: DiffExtent,
) -> Result<Option<FileDiff>, String> {
    facade
        .blocking(move |f| f.git_diff(project, &path, target, extent))
        .await
        .map_err(|err| err.to_string())
}

/// The working tree's copy of one path, for the surface that shows a file rather than a change
/// to one. Bounded: a file past the adapter's ceiling arrives cut and says so, and one holding
/// bytes that are not text arrives with no text at all. `null` for a project that is not a
/// repository, a path outside it, and one that is no longer there.
///
/// Reading a file goes through the repository adapter, so it goes to the blocking pool.
#[tauri::command]
pub async fn git_file(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    path: String,
) -> Result<Option<FileContent>, String> {
    facade
        .blocking(move |f| f.git_file(project, &path))
        .await
        .map_err(|err| err.to_string())
}

/// Whether the user has trusted this project to be changed by Soloist. The rail asks so it can
/// offer the trust affordance rather than let an action fail; a surface that does not ask
/// changes nothing either, because the gate is in the core.
#[tauri::command]
pub async fn git_trusted(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<bool, String> {
    facade
        .blocking(move |f| f.is_project_trusted(project))
        .await
        .map_err(|err| err.to_string())
}

/// Records that trust for a project, which is what the affordance behind [`git_trusted`] does.
#[tauri::command]
pub async fn git_trust_project(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.trust_project(project))
        .await
        .map_err(|err| err.to_string())
}

/// Records everything the working tree holds for one path in the index.
///
/// Changing a repository runs an external tool, so it goes to the blocking pool.
#[tauri::command]
pub async fn git_stage(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    path: String,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_stage(project, &path))
        .await
        .map_err(|err| err.to_string())
}

/// Takes one path back out of the index, leaving the working tree untouched.
#[tauri::command]
pub async fn git_unstage(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    path: String,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_unstage(project, &path))
        .await
        .map_err(|err| err.to_string())
}

/// Throws away what the working tree holds for one path beyond the index. Destructive, so a
/// surface confirms it first; bounded, because it restores from the index and can reach no
/// further back than that.
#[tauri::command]
pub async fn git_discard(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    path: String,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_discard(project, &path))
        .await
        .map_err(|err| err.to_string())
}

/// Records only one hunk of a path's unstaged change in the index. The hunk is named by where
/// it falls, so a request built against a diff the file has moved past is refused rather than
/// applied somewhere else.
#[tauri::command]
pub async fn git_stage_hunk(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    path: String,
    hunk: HunkRange,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_stage_hunk(project, &path, hunk))
        .await
        .map_err(|err| err.to_string())
}

/// Takes only one hunk of a path's staged change back out of the index.
#[tauri::command]
pub async fn git_unstage_hunk(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    path: String,
    hunk: HunkRange,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_unstage_hunk(project, &path, hunk))
        .await
        .map_err(|err| err.to_string())
}

/// Throws away only one hunk of a path's unstaged change. Destructive, and bounded exactly as
/// [`git_discard`] is.
#[tauri::command]
pub async fn git_discard_hunk(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    path: String,
    hunk: HunkRange,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_discard_hunk(project, &path, hunk))
        .await
        .map_err(|err| err.to_string())
}

/// Records the index as a commit, or replaces the last commit with it when `amend`. The
/// repository's hooks, the user's signing key and their configuration all apply, because it is
/// their own `git` that runs — which is also why a hook can take a moment, and why this goes to
/// the blocking pool.
#[tauri::command]
pub async fn git_commit(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    message: String,
    amend: bool,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_commit(project, &message, amend))
        .await
        .map_err(|err| err.to_string())
}
