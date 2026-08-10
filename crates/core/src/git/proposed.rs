//! What a branch proposes — the commits it holds that its base does not — read once for everything
//! that describes them.
//!
//! **What a branch proposes is its commits, not its diff.** They are already somebody's account of
//! their own work, they are short, and they are in order. Only what the branch holds and its base
//! does not is read: a base merged in along the way is not part of what is being proposed.
//!
//! Every refusal that can be reached before anything is described lives here, so the prompt an agent
//! is given and the suggestion computed from the same commits refuse identically rather than nearly
//! identically.

use std::path::Path;

use crate::ids::ProjectId;
use crate::vcs::CommitEntry;

use super::branch::usable_branch_name;
use super::pr::PullRequestError;
use super::repository::LogRange;
use super::status::Git;

/// How many of the branch's commits are read. Enough to describe any branch somebody opens a pull
/// request from by hand, and a ceiling on a branch that has been running for months.
const PROPOSED_COMMITS: usize = 50;

/// A branch and what it proposes. The commits are never empty: a branch holding nothing its base
/// does not proposes nothing, and is refused before one of these is made.
pub(super) struct Proposed {
    /// The branch that is checked out, which is the one whose commits are proposed — read from the
    /// repository rather than accepted from a caller.
    pub head: String,
    /// What it holds that its base does not, newest first, at most [`PROPOSED_COMMITS`] of them.
    pub commits: Vec<CommitEntry>,
}

impl Git {
    /// What `project`'s checked-out branch proposes against `base`.
    ///
    /// [`PullRequestError::UnusableBranchName`] for a base version control would read as an option,
    /// judged before the repository is read at all since it is the caller's own input;
    /// [`PullRequestError::DetachedHead`] where nothing is checked out by name; and
    /// [`PullRequestError::NothingToDescribe`] for a branch holding nothing its base does not.
    ///
    /// A read, so it is ungated. It runs an external tool, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub(super) fn proposed(
        &self,
        project: ProjectId,
        root: &Path,
        base: &str,
    ) -> Result<Proposed, PullRequestError> {
        if !usable_branch_name(base) {
            return Err(PullRequestError::UnusableBranchName);
        }
        let head = self
            .status(project, root)?
            .and_then(|status| status.into_facts().branch.name)
            .ok_or(PullRequestError::DetachedHead)?;
        let commits = self
            .history(project, root, LogRange::Since { base }, 0, PROPOSED_COMMITS)?
            .unwrap_or_default();
        if commits.is_empty() {
            return Err(PullRequestError::NothingToDescribe);
        }
        Ok(Proposed { head, commits })
    }
}

/// One line per commit, newest first, within `budget` — dropping the oldest first, since the newest
/// are what the branch most recently became.
pub(super) fn commit_lines(commits: &[CommitEntry], budget: usize) -> String {
    let mut listed = String::new();
    for commit in commits {
        let line = format!("- {}\n", commit.subject);
        if listed.len() + line.len() > budget {
            break;
        }
        listed.push_str(&line);
    }
    listed
}
