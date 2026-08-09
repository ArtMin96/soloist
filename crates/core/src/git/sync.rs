//! Exchanging commits with a remote, and abandoning a merge that came back from one.
//!
//! These are the only operations in this context that reach a machine Soloist has no say over, and
//! that shapes every rule they follow. They pass the project's trust gate: a remote's address, and
//! the helper program that might be asked for a credential for it, are both configuration the
//! project carries, so running one runs what the project says to. They run under the project's gate,
//! so a fetch that will not finish for a minute is never joined by a second one. They can be
//! **stopped**, because a remote that accepts a connection and then says nothing would otherwise be
//! waited out to the limit. And whether one may stop and ask a person for a credential is
//! [`Prompting`], decided by which façade the caller holds rather than by anything here.
//!
//! Nothing here chooses how a divergence should be reconciled, and nothing here names a credential
//! helper. Whether a pull merges or rebases is the user's own configuration, and where they have not
//! said, version control refuses and says so — which is passed on, because guessing would rewrite
//! history on their behalf.

use std::path::Path;

use crate::ids::ProjectId;

use super::error::GitWriteError;
use super::exchange::{Progress, Prompting, SyncOp};
use super::repository::Exchange;
use super::status::Git;

impl Git {
    /// Hands the checked-out branch's commits to its remote.
    ///
    /// A branch that tracks nothing has no upstream to hand them to, so this publishes it instead —
    /// the same intent, and the only thing that could be meant. The choice is made from the
    /// remembered status rather than asked for, so no surface has to know the difference; a surface
    /// that wants to *say* which one it is asks the status the same question.
    pub fn push(
        &self,
        project: ProjectId,
        root: &Path,
        prompting: Prompting,
        progress: &Progress,
    ) -> Result<(), GitWriteError> {
        let tracking = self
            .status(project, root)?
            .is_some_and(|status| status.branch.upstream.is_some());
        let op = if tracking {
            SyncOp::Push
        } else {
            SyncOp::Publish
        };
        self.exchange(project, root, op, prompting, progress)
    }

    /// Brings the remote's commits in and reconciles them with what is checked out, however the
    /// user's own configuration says to.
    pub fn pull(
        &self,
        project: ProjectId,
        root: &Path,
        prompting: Prompting,
        progress: &Progress,
    ) -> Result<(), GitWriteError> {
        self.exchange(project, root, SyncOp::Pull, prompting, progress)
    }

    /// Brings the remote's commits in without touching the working tree, which is what makes the
    /// standing against the upstream true again.
    pub fn fetch(
        &self,
        project: ProjectId,
        root: &Path,
        prompting: Prompting,
        progress: &Progress,
    ) -> Result<(), GitWriteError> {
        self.exchange(project, root, SyncOp::Fetch, prompting, progress)
    }

    /// Abandons a merge that is under way, restoring what was checked out before it began.
    ///
    /// Destructive within the merge: a conflict resolved by hand since it started is thrown away
    /// with it, which is why every surface confirms this before asking for it.
    pub fn abort_merge(&self, project: ProjectId, root: &Path) -> Result<(), GitWriteError> {
        self.mutating(project, |repository| repository.abort_merge(root))
    }

    /// The one route to a remote, so every exchange with one is gated, serialized, bounded and
    /// stoppable alike.
    ///
    /// The signal is armed inside the gate, so what a change of mind reaches is always the exchange
    /// that is actually running — never one still waiting for the repository to be free, and never
    /// one that has already finished, since the next exchange arms a signal of its own.
    fn exchange(
        &self,
        project: ProjectId,
        root: &Path,
        op: SyncOp,
        prompting: Prompting,
        progress: &Progress,
    ) -> Result<(), GitWriteError> {
        self.mutating(project, |repository| {
            let stop = self.arm(project);
            repository.sync(
                root,
                Exchange {
                    op,
                    prompting,
                    stop: &stop,
                    progress,
                },
            )
        })
    }
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
