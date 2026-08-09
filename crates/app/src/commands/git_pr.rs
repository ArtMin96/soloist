//! The pull-request commands: thin wrappers that route to the one core method.
//!
//! No logic here — not the trust gate, not which description skeleton wins, not whether the branch
//! has to be pushed first. All three are the core's, so a surface that reaches pull requests later
//! is answered by the same rules rather than by a check this one remembers to make. These are the
//! **local user's** door, which is why the push the core makes first may stop and ask them for a
//! credential: somebody clicked something and is sitting in front of it.

use std::sync::Arc;

use soloist_core::{Facade, NewPullRequest, ProjectId, PullRequestSurface};
use tauri::State;

/// Everything the pull-request surface needs to decide what to show: whether the GitHub
/// command-line tool can be reached at all, the branch that would be proposed, the branch it would
/// merge into, the pull request the branch already has, and the description skeletons on offer.
///
/// Reaches another machine, so it goes to the blocking pool.
#[tauri::command]
pub async fn git_pull_request_surface(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<PullRequestSurface, String> {
    facade
        .blocking(move |f| f.git_pull_request_surface(project))
        .await
        .map_err(|err| err.to_string())
}

/// Proposes what is checked out as a pull request, publishing the branch first when the remote
/// does not hold it as it stands. Answers with the address of what was made, which is what the
/// service itself reports; the surface re-reads to show the rest.
///
/// Pushes and then reaches another machine, so it goes to the blocking pool — and it is stoppable
/// through [`git_stop_exchange`](super::git_stop_exchange), which reaches both halves.
#[tauri::command]
pub async fn git_create_pull_request(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    request: NewPullRequest,
) -> Result<String, String> {
    facade
        .blocking(move |f| f.git_create_pull_request(project, &request))
        .await
        .map_err(|err| err.to_string())
}

/// Drafts the description for the pull request this branch would open against `base`, filling
/// `skeleton` when one was offered, by running the agent tool the user picked once, headless. Only
/// text comes back: nothing here proposes anything, and the caller is expected to read and change
/// it first.
///
/// Refused outright until a tool is selected in settings, and until the project is trusted.
/// Reading the repository and running the tool are both off the runtime, which the core arranges:
/// this awaits the one method.
#[tauri::command]
pub async fn git_draft_pull_request_body(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    base: String,
    skeleton: String,
) -> Result<String, String> {
    facade
        .git_draft_pull_request_body(project, base, skeleton)
        .await
        .map_err(|err| err.to_string())
}
