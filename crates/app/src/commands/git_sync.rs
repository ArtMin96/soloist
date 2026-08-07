//! The version-control commands that reach a remote, and the one that abandons a merge one of them
//! left behind: thin wrappers that route to the one core method.
//!
//! No logic here — not the trust gate, not which exchange a branch without an upstream needs, and
//! not whether a person may be asked for a credential. All three are the core's, so an agent over
//! MCP is answered by the same rules rather than by a check this surface remembers to make. These
//! are the **local user's** door, which is exactly why the core lets a credential prompt through on
//! them: somebody clicked something and is sitting in front of it.

use std::sync::Arc;

use soloist_core::{Facade, ProjectId};
use tauri::State;

/// Hands the checked-out branch's commits to its remote, publishing the branch when it tracks
/// nothing yet — which one it is comes from the repository's own state, not from here.
///
/// Reaches another machine under the user's own credentials, bounded by the adapter's limit for
/// doing so and stoppable before then ([`git_stop_exchange`]), so it goes to the blocking pool.
#[tauri::command]
pub async fn git_push(facade: State<'_, Arc<Facade>>, project: ProjectId) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_push(project))
        .await
        .map_err(|err| err.to_string())
}

/// Brings the remote's commits in and reconciles them with what is checked out, however the user's
/// own configuration says to. Where they have not said, version control's own refusal comes back.
#[tauri::command]
pub async fn git_pull(facade: State<'_, Arc<Facade>>, project: ProjectId) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_pull(project))
        .await
        .map_err(|err| err.to_string())
}

/// Brings the remote's commits in without touching the working tree, which is what makes the
/// standing against the upstream true again.
#[tauri::command]
pub async fn git_fetch(facade: State<'_, Arc<Facade>>, project: ProjectId) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_fetch(project))
        .await
        .map_err(|err| err.to_string())
}

/// Asks the exchange with a remote running against this project to stop.
///
/// Deliberately **not** on the blocking pool: it sets a signal the exchange looks at, so it must
/// answer while that exchange is still running — and the pool is exactly where the exchange is
/// sitting. Nothing to stop is not an error.
#[tauri::command]
pub fn git_stop_exchange(facade: State<'_, Arc<Facade>>, project: ProjectId) {
    facade.git_stop_exchange(project);
}

/// Abandons a merge that is under way, restoring what was checked out before it began. Destructive
/// within that merge — a conflict resolved by hand goes with it — so a surface confirms it first.
#[tauri::command]
pub async fn git_abort_merge(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.git_abort_merge(project))
        .await
        .map_err(|err| err.to_string())
}
