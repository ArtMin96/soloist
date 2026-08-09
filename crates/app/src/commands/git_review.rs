//! The pull-request review commands: thin wrappers that route to the one core method.
//!
//! No logic here — not the trust gate the merge spends, not which agent a handoff reaches, not what
//! it says. All three are the core's, so a surface that reaches review later is answered by the
//! same rules rather than by a check this one remembers to make.

use std::sync::Arc;

use soloist_core::{
    Facade, Handoff, HandoffSubject, MergeMethod, ProcessId, Progress, ProjectId, PullRequestReview,
};
use tauri::State;

/// What the checked-out branch has open on the service: the pull request, what its checks say, and
/// what people have written on it. `null` when the branch has nothing open.
///
/// Reaches another machine, so it goes to the blocking pool.
#[tauri::command]
pub async fn git_pull_request_review(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
) -> Result<Option<PullRequestReview>, String> {
    facade
        .blocking(move |f| f.git_pull_request_review(project))
        .await
        .map_err(|err| err.to_string())
}

/// Puts a pull request's commits into its base branch. What the service refuses — a check that has
/// not passed, a review that is owed — comes back in its own words and nothing is merged.
///
/// Reaches another machine, so it goes to the blocking pool, and it is stoppable through
/// [`git_stop_exchange`](super::git_stop_exchange).
#[tauri::command]
pub async fn git_merge_pull_request(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    number: u64,
    method: MergeMethod,
) -> Result<(), String> {
    facade
        .blocking(move |f| {
            f.git_merge_pull_request(project, number, method, &Progress::unwatched())
        })
        .await
        .map_err(|err| err.to_string())
}

/// Hands what a check or a conversation says to an agent, as text in its session. `target` names
/// which agent, and omitting it asks for the project's only running one; with none running the text
/// comes back to be copied instead.
///
/// **Nothing is submitted.** The context lands where a paste would land, and pressing return stays
/// the reader's decision.
#[tauri::command]
pub async fn git_hand_off(
    facade: State<'_, Arc<Facade>>,
    project: ProjectId,
    subject: HandoffSubject,
    target: Option<ProcessId>,
) -> Result<Handoff, String> {
    facade
        .git_hand_off(project, subject, target)
        .await
        .map_err(|err| err.to_string())
}
