//! Turning what the tool reports about an open pull request into the core's review vocabulary:
//! the checks the service ran, and everything anybody wrote that is not attached to a line.
//!
//! The service reports a check as two words out of two published enumerations, or — for the older
//! commit-status kind — as one word out of a third. All three are mapped here onto the five states
//! a reader acts on, so nothing downstream ever sees the service's own spelling. A word none of the
//! three names is [`CheckState::Unknown`] rather than a failure of the whole read: a pull request
//! carries checks by the dozen, and one the service has newly learnt to say must not take the other
//! twenty off the surface with it.

use serde::Deserialize;

use soloist_core::{CheckRun, CheckState, ReviewComment, ReviewThread};

/// The fields one pull request's review is asked for, on top of the pull request's own. Requested
/// as one string and read into [`ReviewPayload`], so the request and the reading are the same list.
pub(crate) const REVIEW_FIELDS: &str = "statusCheckRollup,reviews,comments";

/// The one status meaning "it reached an end". Every other value of that enumeration means it has
/// not, so they are read as one rather than named one at a time.
const COMPLETED: &str = "COMPLETED";

/// What a completed check may conclude, in the service's own spelling.
const SUCCESS: &str = "SUCCESS";
const NEUTRAL: &str = "NEUTRAL";
const FAILURE: &str = "FAILURE";
const TIMED_OUT: &str = "TIMED_OUT";
const ACTION_REQUIRED: &str = "ACTION_REQUIRED";
const STARTUP_FAILURE: &str = "STARTUP_FAILURE";
const SKIPPED: &str = "SKIPPED";
const STALE: &str = "STALE";
const CANCELLED: &str = "CANCELLED";

/// What a commit status may report, in the service's own spelling.
const PENDING: &str = "PENDING";
const EXPECTED: &str = "EXPECTED";
const ERROR: &str = "ERROR";

/// What the tool reports about an open pull request beyond the pull request itself.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewPayload {
    pub status_check_rollup: Vec<RollupEntry>,
    pub reviews: Vec<ReviewPayloadEntry>,
    pub comments: Vec<CommentPayload>,
}

/// One entry of the check rollup. The two kinds share a field name for neither their name nor their
/// state, so they are read as two shapes told apart by the kind the service stamps on each.
#[derive(Deserialize)]
#[serde(tag = "__typename")]
pub(crate) enum RollupEntry {
    #[serde(rename = "CheckRun", rename_all = "camelCase")]
    Run {
        name: String,
        status: String,
        conclusion: Option<String>,
        workflow_name: Option<String>,
        details_url: Option<String>,
    },
    #[serde(rename = "StatusContext", rename_all = "camelCase")]
    Status {
        context: String,
        state: String,
        target_url: Option<String>,
    },
    /// A kind neither of the two above. Carried rather than refused, for the same reason an
    /// unrecognised conclusion is.
    #[serde(other)]
    Other,
}

/// One review somebody submitted. It has no address of its own on the service, which is why a
/// thread's address is optional.
#[derive(Deserialize)]
pub(crate) struct ReviewPayloadEntry {
    id: String,
    author: Option<Author>,
    body: String,
}

/// One remark on the pull request itself.
#[derive(Deserialize)]
pub(crate) struct CommentPayload {
    id: String,
    author: Option<Author>,
    body: String,
    url: String,
}

/// Who wrote something. Absent where the account has since been deleted, which the service reports
/// as no author at all rather than as an error.
#[derive(Deserialize)]
pub(crate) struct Author {
    login: String,
}

/// What an author with no account left is shown as, so a comment still reads as somebody's.
const NOBODY: &str = "(unknown)";

impl RollupEntry {
    /// This entry as a check, or `None` for a kind this does not know — dropped rather than shown
    /// as a check whose name and state were guessed at.
    pub(crate) fn check(self) -> Option<CheckRun> {
        match self {
            RollupEntry::Run {
                name,
                status,
                conclusion,
                workflow_name,
                details_url,
            } => Some(CheckRun {
                name,
                state: run_state(&status, conclusion.as_deref()),
                workflow: workflow_name.filter(|workflow| !workflow.is_empty()),
                url: details_url,
            }),
            RollupEntry::Status {
                context,
                state,
                target_url,
            } => Some(CheckRun {
                name: context,
                state: status_state(&state),
                workflow: None,
                url: target_url,
            }),
            RollupEntry::Other => None,
        }
    }
}

/// Where a check run stands: nothing is concluded until it says it reached an end, and what it
/// concluded then decides the rest.
fn run_state(status: &str, conclusion: Option<&str>) -> CheckState {
    if status != COMPLETED {
        return CheckState::Pending;
    }
    match conclusion {
        // Concluded with nothing to conclude. Not a state the service documents as reachable, and
        // read as still pending rather than as a verdict nobody gave.
        None => CheckState::Pending,
        Some(SUCCESS | NEUTRAL) => CheckState::Passed,
        Some(FAILURE | TIMED_OUT | ACTION_REQUIRED | STARTUP_FAILURE) => CheckState::Failed,
        // A stale answer belongs to commits that have been replaced, so it judges nothing about
        // what is proposed now — the same as never having run.
        Some(SKIPPED | STALE) => CheckState::Skipped,
        Some(CANCELLED) => CheckState::Cancelled,
        Some(_) => CheckState::Unknown,
    }
}

/// Where a commit status stands. Its enumeration is smaller and has no separate notion of having
/// finished — the state is the whole answer.
fn status_state(state: &str) -> CheckState {
    match state {
        SUCCESS => CheckState::Passed,
        PENDING | EXPECTED => CheckState::Pending,
        FAILURE | ERROR => CheckState::Failed,
        _ => CheckState::Unknown,
    }
}

impl ReviewPayloadEntry {
    /// This review as a thread about the change as a whole, or `None` when it carries no words —
    /// a review submitted with nothing written on it says nothing a reader needs.
    fn thread(self) -> Option<ReviewThread> {
        (!self.body.trim().is_empty()).then(|| ReviewThread {
            id: self.id,
            url: None,
            path: None,
            line: None,
            resolved: false,
            outdated: false,
            comments: vec![ReviewComment {
                author: author(self.author),
                body: self.body,
                url: None,
            }],
        })
    }
}

impl CommentPayload {
    /// This remark as a thread about the change as a whole.
    fn thread(self) -> ReviewThread {
        ReviewThread {
            id: self.id,
            url: Some(self.url.clone()),
            path: None,
            line: None,
            resolved: false,
            outdated: false,
            comments: vec![ReviewComment {
                author: author(self.author),
                body: self.body,
                url: Some(self.url),
            }],
        }
    }
}

impl ReviewPayload {
    /// The checks this pull request carries, and the conversations that hang on the pull request
    /// rather than on a line of it — reviews before remarks, since a review is somebody's account
    /// of the whole change.
    pub(crate) fn into_parts(self) -> (Vec<CheckRun>, Vec<ReviewThread>) {
        let checks = self
            .status_check_rollup
            .into_iter()
            .filter_map(RollupEntry::check)
            .collect();
        let threads = self
            .reviews
            .into_iter()
            .filter_map(ReviewPayloadEntry::thread)
            .chain(self.comments.into_iter().map(CommentPayload::thread))
            .collect();
        (checks, threads)
    }
}

fn author(author: Option<Author>) -> String {
    author.map_or_else(|| NOBODY.to_string(), |author| author.login)
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
