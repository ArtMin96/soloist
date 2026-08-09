//! What a pull request looks like once it is open: what the service's checks say about it, what
//! people have written on it, and putting it into the base branch.
//!
//! The vocabulary here is deliberately smaller than the service's. A check reports a status and a
//! conclusion drawn from two published enumerations of nine and six words; a reader needs to know
//! whether it is still running, whether it objected, and whether it declined to judge. So the
//! adapter maps those words onto [`CheckState`] and nothing here ever sees them.
//!
//! Everything a person wrote arrives as a [`ReviewThread`], whether it hangs off a line of the diff
//! or off the pull request itself. One shape rather than three means a surface renders one list and
//! a handoff quotes one thing, and the only difference between them is whether the thread knows
//! where in the diff it belongs.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ids::ProjectId;

use super::forge::PullRequest;
use super::pr::PullRequestError;
use super::status::Git;

/// How many threads one read carries, and how many comments of each. A pull request under long
/// argument is the ordinary case rather than the pathological one, so both are bounded: what a
/// reader can take in is far less than what a service will send, and an unbounded read of somebody
/// else's discussion is an unbounded allocation.
pub const REVIEW_LIMITS: ReviewLimits = ReviewLimits {
    threads: 50,
    comments: 20,
};

/// How much of one read is carried back, handed to the port so the bound is the core's and an
/// adapter cannot quietly widen it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReviewLimits {
    /// The most threads carried, newest discussion first where the service orders them.
    pub threads: usize,
    /// The most comments carried within one thread.
    pub comments: usize,
}

/// Where one check stands, in the four answers a reader acts on differently plus the two that mean
/// nothing is owed.
///
/// A closed set the adapter maps onto, never the service's own words. It is deliberately coarser
/// than what a service reports: "timed out", "action required" and "failed to start" are all
/// [`CheckState::Failed`] to somebody deciding whether to merge.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    /// Queued, waiting, or still running — nothing has been concluded yet.
    Pending,
    /// It concluded and did not object.
    Passed,
    /// It concluded against the change.
    Failed,
    /// It declined to run, or its answer belongs to a commit that has been superseded.
    Skipped,
    /// Somebody stopped it before it concluded.
    Cancelled,
    /// It reported something this build does not recognise.
    ///
    /// Deliberately not a failure of the whole read, which is where a pull request's own state
    /// differs: that is the single fact a read is about, while checks arrive by the dozen, and one
    /// the service has newly learnt to say must not take the other twenty off the surface with it.
    Unknown,
}

impl CheckState {
    /// Whether this state is one a reader would want to act on — the checks a handoff is offered
    /// for, so no surface decides that for itself.
    pub fn objecting(self) -> bool {
        matches!(self, CheckState::Failed)
    }
}

/// One check the service ran against the pull request's commits.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CheckRun {
    /// What the service calls it, which is also how a caller names it when asking for a handoff.
    pub name: String,
    pub state: CheckState,
    /// The larger run it belongs to, where the service says — several checks of one workflow read
    /// as one thing to a person.
    pub workflow: Option<String>,
    /// Where it can be read in full, or `None` where the service offered no address. Opaque here:
    /// what an address means is the adapter's business, and it is the only place a service's own
    /// host appears.
    pub url: Option<String>,
}

/// One thing a person wrote.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReviewComment {
    /// Who wrote it, as the service names them.
    pub author: String,
    /// What they wrote, in the markup they wrote it in — carried whole, rendered by the surface.
    pub body: String,
    /// Where it can be read on the service, or `None` where the service publishes no address for
    /// it — a submitted review's own summary has none.
    pub url: Option<String>,
}

/// One conversation on the pull request: a line of the diff somebody objected to, a review
/// somebody submitted, or a remark on the pull request itself.
///
/// The three are one type because a reader treats them as one list and a handoff quotes them the
/// same way. What tells them apart is [`ReviewThread::path`]: a thread that knows where in the diff
/// it belongs can say so, and one that does not is about the change as a whole.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ReviewThread {
    /// What the service files it under, which is what a caller names when asking for a handoff.
    /// Opaque: it is an identifier the service issued, never anything composed here.
    pub id: String,
    /// Where it can be read on the service, or `None` where the service publishes no address.
    pub url: Option<String>,
    /// The file it hangs on, or `None` for a remark about the change as a whole.
    pub path: Option<String>,
    /// The line it hangs on, or `None` where the service no longer places it — a thread whose lines
    /// have moved keeps its file and loses its line.
    pub line: Option<u64>,
    /// Whether somebody has marked it settled. Settled discussion is kept out of the way rather
    /// than thrown away, because the argument is often what explains the code.
    pub resolved: bool,
    /// Whether it hangs on a version of the diff that has since been replaced.
    pub outdated: bool,
    pub comments: Vec<ReviewComment>,
}

/// Everything an open pull request's review surface renders, in one read.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PullRequestReview {
    /// The pull request itself, re-read with the rest so a surface polling this alone still sees it
    /// close, merge, or leave draft.
    pub pull_request: PullRequest,
    pub checks: Vec<CheckRun>,
    pub threads: Vec<ReviewThread>,
}

/// How a pull request's commits are put into its base branch. The three the service offers, named
/// rather than left to a default, because which one a repository wants is a decision it has already
/// made and a caller has to be told.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMethod {
    /// Keep every commit and record a merge.
    Merge,
    /// Fold them into one commit on the base.
    Squash,
    /// Replay them onto the base with no merge recorded.
    Rebase,
}

impl Git {
    /// What `project`'s checked-out branch has open on the service, with what the checks say and
    /// what people have written — or `None` when the branch has nothing open.
    ///
    /// A read, so it is ungated. It reaches another machine, so callers come from
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker; it runs
    /// under the project's gate, so it never overlaps a change to the same repository.
    pub fn pull_request_review(
        &self,
        project: ProjectId,
        root: &Path,
    ) -> Result<Option<PullRequestReview>, PullRequestError> {
        let Some(head) = self.review_head(project, root)? else {
            return Ok(None);
        };
        Ok(self.asking(project, |forge, _| forge.review(root, &head, REVIEW_LIMITS))?)
    }

    /// Puts `number`'s commits into its base branch by `method`.
    ///
    /// Gated on the user having trusted the project, like every other change: it rewrites a branch
    /// everybody else is working from. The service refuses what its own rules refuse — a check that
    /// has not passed, a review that is owed — and that refusal comes back in the service's own
    /// words rather than being pre-empted here, because the rules are the repository's and nothing
    /// local can know them.
    pub fn merge_pull_request(
        &self,
        project: ProjectId,
        root: &Path,
        number: u64,
        method: MergeMethod,
    ) -> Result<(), PullRequestError> {
        if !self.trusted(project)? {
            return Err(PullRequestError::Untrusted);
        }
        Ok(self.asking(project, |forge, stop| {
            forge.merge(root, number, method, stop)
        })?)
    }

    /// The branch a review is read for: what is checked out by name, or `None` for a detached head,
    /// which has nothing anybody could have proposed.
    fn review_head(
        &self,
        project: ProjectId,
        root: &Path,
    ) -> Result<Option<String>, PullRequestError> {
        Ok(self
            .status(project, root)?
            .and_then(|status| status.branch.name))
    }
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
