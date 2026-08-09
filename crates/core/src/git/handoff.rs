//! Turning one thing a reviewer or a check said into something an agent can be told.
//!
//! This composes text and nothing else. It runs no tool on the agent's behalf, submits nothing, and
//! decides nothing about what should be done — what comes out is context, clearly fenced, for a
//! person or an agent to read and act on. That is the whole contract: a one-way paste.
//!
//! What goes in is bounded twice over, because everything here was written by somebody else. A
//! check's log is taken from its end and capped; a thread carries a bounded number of comments;
//! and the whole block is composed to a ceiling rather than cut down to one, so nothing is ever
//! handed over as half a sentence.

use serde::{Deserialize, Serialize};

use std::path::Path;

use crate::ids::ProjectId;

use super::pr::PullRequestError;
use super::review::{CheckRun, PullRequestReview, ReviewThread, REVIEW_LIMITS};
use super::status::Git;

/// The most a composed handoff carries. Generous enough for a failing build's own account of
/// itself, bounded because this is delivered into somebody's session as one paste.
pub const HANDOFF_LIMIT: usize = 16 * 1024;

/// The most of a check's log that is carried. The rest of the ceiling above belongs to what the
/// pull request and the check are, which is what makes the excerpt legible at all.
pub const CHECK_LOG_LIMIT: usize = 12 * 1024;

/// The fence the context is written between, so an agent reading it can tell quoted material from
/// the rest of its session. Plain and self-describing, since it is read by people too.
const OPENING: &str = "--- context from Soloist (nothing has been run) ---";
const CLOSING: &str = "--- end of context ---";

/// What a handoff is about — named by what a surface can point at rather than carried as text, so
/// nothing a caller composed itself can be delivered as though the service had said it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum HandoffSubject {
    /// A check, by the name the service reports it under.
    Check { name: String },
    /// A conversation, by what the service files the thread under.
    Thread { id: String },
}

impl Git {
    /// The context block for `subject` on `project`'s open pull request.
    ///
    /// Composed from a fresh read rather than from anything a caller supplied, so what is handed
    /// over is what the service says now — a check that has since passed composes as one that
    /// passed. A read, so it is ungated.
    pub fn handoff_context(
        &self,
        project: ProjectId,
        root: &Path,
        subject: &HandoffSubject,
    ) -> Result<String, PullRequestError> {
        let review = self
            .pull_request_review(project, root)?
            .ok_or(PullRequestError::NoPullRequest)?;
        match subject {
            HandoffSubject::Check { name } => {
                let check = review
                    .checks
                    .iter()
                    .find(|check| &check.name == name)
                    .ok_or(PullRequestError::NoSuchSubject)?;
                let log = self.asking(project, |forge, _| {
                    forge.check_log(root, check, CHECK_LOG_LIMIT)
                })?;
                Ok(check_context(&review, check, log.as_deref()))
            }
            HandoffSubject::Thread { id } => {
                let thread = review
                    .threads
                    .iter()
                    .find(|thread| &thread.id == id)
                    .ok_or(PullRequestError::NoSuchSubject)?;
                Ok(thread_context(&review, thread))
            }
        }
    }
}

/// What an agent is told about a check that did not pass: which pull request it belongs to, what
/// the check is, and as much of the end of its output as there is room for.
fn check_context(review: &PullRequestReview, check: &CheckRun, log: Option<&str>) -> String {
    let mut block = Fenced::new();
    block.line(&format!(
        "Pull request #{}: {}",
        review.pull_request.number, review.pull_request.title
    ));
    block.line(&format!(
        "Branch {} into {}",
        review.pull_request.head, review.pull_request.base
    ));
    block.line(&format!("Failing check: {}", check.name));
    if let Some(workflow) = &check.workflow {
        block.line(&format!("Workflow: {workflow}"));
    }
    if let Some(url) = &check.url {
        block.line(&format!("Details: {url}"));
    }
    match log {
        Some(log) => {
            block.line("");
            block.line("End of its output:");
            block.line(log);
        }
        // Said outright rather than left as a silence, so nobody reads a check with no reachable
        // output as one that printed nothing.
        None => block.line("Its output is not reachable from here."),
    }
    block.finish()
}

/// What an agent is told about a comment: where in the change it hangs, and what everybody in the
/// thread said, oldest first, because a thread only reads correctly in order.
fn thread_context(review: &PullRequestReview, thread: &ReviewThread) -> String {
    let mut block = Fenced::new();
    block.line(&format!(
        "Pull request #{}: {}",
        review.pull_request.number, review.pull_request.title
    ));
    match (&thread.path, thread.line) {
        (Some(path), Some(line)) => block.line(&format!("Comment on {path}:{line}")),
        (Some(path), None) => block.line(&format!("Comment on {path}")),
        (None, _) => block.line("Comment on the pull request"),
    }
    if thread.outdated {
        block.line("It hangs on a version of the diff that has since changed.");
    }
    if let Some(url) = &thread.url {
        block.line(&format!("Link: {url}"));
    }
    for comment in thread.comments.iter().take(REVIEW_LIMITS.comments) {
        block.line("");
        block.line(&format!("{} wrote:", comment.author));
        block.line(&comment.body);
    }
    block.finish()
}

/// The block being composed: lines are added while there is room for them and dropped whole when
/// there is not, so what comes out is always complete lines within [`HANDOFF_LIMIT`] rather than a
/// sentence cut in half.
struct Fenced {
    text: String,
    full: bool,
}

impl Fenced {
    fn new() -> Self {
        Self {
            text: format!("{OPENING}\n"),
            full: false,
        }
    }

    fn line(&mut self, line: &str) {
        if self.full {
            return;
        }
        if self.text.len() + line.len() + CLOSING.len() + 2 > HANDOFF_LIMIT {
            self.full = true;
            return;
        }
        self.text.push_str(line);
        self.text.push('\n');
    }

    fn finish(mut self) -> String {
        self.text.push_str(CLOSING);
        self.text
    }
}

#[cfg(test)]
#[path = "handoff_tests.rs"]
mod tests;
