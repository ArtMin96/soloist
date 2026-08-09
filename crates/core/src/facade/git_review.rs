//! The review commands and queries adapters call (context C8): what an open pull request looks
//! like, putting it into its base branch, and handing one of its objections to an agent.
//!
//! The handoff is the one behaviour here that reaches outside version control, and it is split on
//! purpose: the git context composes the text, this façade routes it to a process. Neither half
//! knows the other — C9 never learns what a process is, and supervision never learns what a pull
//! request is — and what crosses between them is a string.
//!
//! Nothing here runs anything on an agent's behalf. The context arrives in the session as text, the
//! way a person pasting it would leave it, and what to do about it is the reader's decision.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::Facade;
use crate::git::{HandoffSubject, MergeMethod, PullRequestError, PullRequestReview};
use crate::ids::{ProcessId, ProjectId};
use crate::process::{ProcStatus, ProcessKind};
use crate::supervisor::SupervisorError;

/// What became of a handoff.
///
/// Delivering it and having nowhere to deliver it are both ordinary answers rather than one being a
/// failure: an agent is something the user starts when they want one, so the case where none is
/// running is the case this has to be good at.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "delivery")]
pub enum Handoff {
    /// It went into the session of the agent named here, as text nobody has submitted.
    Delivered { process: ProcessId, text: String },
    /// No agent was running to take it, so the text comes back for the user to do with as they
    /// like. Never a silent no-op, and never a failure: there was simply nowhere to put it.
    Copy { text: String },
}

/// Why a handoff produced nothing at all.
#[derive(Debug, thiserror::Error)]
pub enum HandoffError {
    /// The process named to deliver to is not a running agent of this project.
    #[error("that is not a running agent in this project")]
    NotAnAgent,
    /// The context itself could not be composed — no pull request, no such check or conversation,
    /// or the forge could not be reached.
    #[error(transparent)]
    Context(#[from] PullRequestError),
    /// The agent was there when it was chosen and is not there now.
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
}

impl Facade {
    /// What `project`'s checked-out branch has open on the service: the pull request, what the
    /// checks say, and what people have written — or `None` when it has nothing open.
    ///
    /// A read, so it is ungated. It reaches another machine, so callers come through
    /// [`Facade::blocking`] rather than a runtime worker.
    pub fn git_pull_request_review(
        &self,
        project: ProjectId,
    ) -> Result<Option<PullRequestReview>, PullRequestError> {
        let root = self.review_root(project)?;
        self.git.pull_request_review(project, &root)
    }

    /// Puts pull request `number`'s commits into its base branch by `method`.
    ///
    /// Gated in the core on the user having trusted the project. What the service refuses — a check
    /// that has not passed, a review that is owed — comes back in its own words and nothing is
    /// merged. Afterwards the working tree's standing against its upstream has moved, so the status
    /// is re-read and announced the way every other change announces itself.
    pub fn git_merge_pull_request(
        &self,
        project: ProjectId,
        number: u64,
        method: MergeMethod,
    ) -> Result<(), PullRequestError> {
        let root = self.review_root(project)?;
        self.git
            .merge_pull_request(project, &root, number, method)?;
        self.announce_git(project, &root);
        Ok(())
    }

    /// Hands what `subject` says on `project`'s pull request to an agent, as text in its session.
    ///
    /// `target` names which agent, and `None` asks for the project's only running one. With no
    /// agent running — or with several and none named — the text comes back to be copied instead,
    /// which is an answer rather than a failure.
    ///
    /// Composing reaches the service, so callers come through [`Facade::blocking`]; delivery is a
    /// write to a terminal that is already open and costs nothing. **Nothing is submitted**: the
    /// context lands where a person's paste would land, and pressing return stays the reader's
    /// decision.
    pub async fn git_hand_off(
        &self,
        project: ProjectId,
        subject: HandoffSubject,
        target: Option<ProcessId>,
    ) -> Result<Handoff, HandoffError> {
        let root = self.review_root(project)?;
        let text = self.git.handoff_context(project, &root, &subject)?;
        let Some(agent) = self.handoff_target(project, target)? else {
            return Ok(Handoff::Copy { text });
        };
        self.supervisor()
            .write_stdin(agent, text.clone().into_bytes())
            .await?;
        Ok(Handoff::Delivered {
            process: agent,
            text,
        })
    }

    /// Which agent a handoff reaches: the one named, once it is confirmed to be a running agent of
    /// this project, or the project's only running agent when none was named.
    ///
    /// Resolved here rather than by whoever asked, so no surface can deliver into another project's
    /// process by naming it — and so "there is nobody to hand this to" is one answer decided in one
    /// place.
    fn handoff_target(
        &self,
        project: ProjectId,
        target: Option<ProcessId>,
    ) -> Result<Option<ProcessId>, HandoffError> {
        let mut running = self.snapshot().into_iter().filter(|view| {
            view.project == project
                && view.kind == ProcessKind::Agent
                && view.status == ProcStatus::Running
        });
        match target {
            Some(named) => running
                .any(|view| view.id == named)
                .then_some(Some(named))
                .ok_or(HandoffError::NotAnAgent),
            None => {
                let first = running.next().map(|view| view.id);
                // Several running agents and nobody said which: choosing one would be guessing at
                // whose session this belongs in, so it is copied instead and the caller offers the
                // choice.
                Ok(first.filter(|_| running.next().is_none()))
            }
        }
    }

    /// The folder a project's repository is read from, refusing a project that is not open.
    fn review_root(&self, project: ProjectId) -> Result<PathBuf, PullRequestError> {
        self.project_root(project)?
            .ok_or(PullRequestError::UnknownProject)
    }
}

#[cfg(test)]
#[path = "git_review_tests.rs"]
mod tests;
