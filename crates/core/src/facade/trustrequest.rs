//! The local user's authority over trust: reading what has been asked, deciding it, and taking
//! back what was granted.
//!
//! Every method here is on [`Facade`] and none is on
//! [`ScopedFacade`](super::scoped::ScopedFacade). That split *is* the security argument: a session
//! reaching the core over MCP is another agent, not the person at the keyboard, so a caller that
//! could approve its own request would be gated by nothing at all. The same reason keeps approve
//! and deny off the IPC surface entirely.

use crate::configchange::TrustReviewCommand;
use crate::coordination::AgentMessageKind;
use crate::hash::{Hash, HashParseError};
use crate::ids::{ProjectId, TrustRequestId};
use crate::ports::{StoreError, TrustGrant};
use crate::process::ProcessKind;
use crate::trustrequest::{TrustRequest, TrustRequestState};

use super::Facade;

/// Why deciding a trust request was refused.
#[derive(Debug, thiserror::Error)]
pub enum ResolveTrustRequestError {
    /// No request under that id is awaiting a decision — it was answered, aged out, or its
    /// requester closed.
    #[error("no trust request under that id is awaiting a decision")]
    NotPending,
    /// The variant the approval would authorize is not the one the request displays. Nothing is
    /// trusted: an approval may only ever grant exactly what was shown.
    #[error("this request no longer authorizes the command that was reviewed")]
    ChangedSinceReview,
    /// The durable grant could not be written, so nothing was trusted.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why revoking a grant was refused.
#[derive(Debug, thiserror::Error)]
pub enum RevokeTrustError {
    /// The variant key was not a well-formed hash, so it names no grant.
    #[error(transparent)]
    UnknownVariant(#[from] HashParseError),
    /// The durable write failed, so the grant still stands.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl Facade {
    /// Every trust request in `project` still awaiting the user's decision.
    pub fn pending_trust_requests(&self, project: ProjectId) -> Vec<TrustRequest> {
        self.trust_requests.pending(project)
    }

    /// Approves a pending request, trusting exactly the variant it displays.
    ///
    /// `reviewed_variant` is the key the surface showing the request had on screen. Three values
    /// must agree before anything is written: the hash re-derived from the pinned spec, the hash
    /// the request's own review carries, and the one the caller reviewed. They cannot disagree
    /// unless something changed between display and decision, and if they do the grant is refused
    /// rather than widened — an approval that authorized a command the user did not read would be
    /// the exact failure this surface exists to prevent.
    ///
    /// On success the grant is durable and carries its provenance, the read model's `requires_trust`
    /// flag clears so the command becomes startable without a restart, and the requester is told:
    /// an agent by a mailbox notice, everyone by
    /// [`trust_request_status`](super::scoped_trust). The notice is sent **after** the grant and
    /// never rolls it back — a full inbox is not a reason to un-decide what the user decided.
    pub fn approve_trust_request(
        &self,
        id: TrustRequestId,
        reviewed_variant: &str,
    ) -> Result<(), ResolveTrustRequestError> {
        let pending = self
            .trust_requests
            .peek(id)
            .ok_or(ResolveTrustRequestError::NotPending)?;
        let variant = pending.spec.variant_hash();
        let granted = variant.to_hex();
        if granted != pending.request.review.variant_hash || granted != reviewed_variant {
            return Err(ResolveTrustRequestError::ChangedSinceReview);
        }
        let project = pending.request.project;
        self.trust.trust_requested(
            project,
            &pending.spec,
            pending.request.requested_by,
            &pending.request.reason,
            self.clock.now_unix_millis(),
        )?;
        self.supervisor.mark_trusted(project, &variant);
        self.trust_requests.resolve(id, TrustRequestState::Granted);
        self.notify_requester(&pending.request, TrustRequestState::Granted);
        Ok(())
    }

    /// Declines a pending request. Nothing is trusted; the requester is told the same two ways an
    /// approval tells it.
    pub fn deny_trust_request(&self, id: TrustRequestId) -> Result<(), ResolveTrustRequestError> {
        let resolved = self
            .trust_requests
            .resolve(id, TrustRequestState::Denied)
            .ok_or(ResolveTrustRequestError::NotPending)?;
        self.notify_requester(&resolved, TrustRequestState::Denied);
        Ok(())
    }

    /// Every command variant trusted in `project`, each with what is known about how it came to be
    /// trusted — a grant with no requester is one the user authored themselves.
    pub fn list_trusted_commands(&self, project: ProjectId) -> Result<Vec<TrustGrant>, StoreError> {
        self.trust.grants(project)
    }

    /// Takes back a grant by its variant key. The supervisor re-checks trust on every start, so a
    /// revoked variant is refused again the next time anything tries to run it — including an
    /// auto-restart or a file-watch restart.
    pub fn revoke_command_trust(
        &self,
        project: ProjectId,
        variant_hash: &str,
    ) -> Result<(), RevokeTrustError> {
        let variant = Hash::from_hex(variant_hash)?;
        self.trust.untrust_variant(project, &variant)?;
        Ok(())
    }

    /// Tells the requesting process what was decided, when it is an agent with an inbox to tell.
    ///
    /// Best-effort by construction, and deliberately the last step: a non-agent requester has no
    /// mailbox at all, and a full one must never undo a grant the user made. The requester is both
    /// sender and recipient because there is no process behind a person's click — the notice is
    /// about the requester's own request.
    fn notify_requester(&self, request: &TrustRequest, outcome: TrustRequestState) {
        let Some(view) = self.process_view(request.requested_by) else {
            return;
        };
        if view.kind != ProcessKind::Agent {
            return;
        }
        let body = decision_notice(&request.review, outcome);
        if self
            .mailbox
            .enqueue(
                request.project,
                request.requested_by,
                request.requested_by,
                AgentMessageKind::TrustDecision,
                body,
                None,
            )
            .is_ok()
            && self.idle.activity(request.requested_by) == Some(crate::agents::AgentActivity::Idle)
        {
            self.mailbox.wake(request.requested_by, self.supervisor());
        }
    }
}

/// The words a decision reaches an agent as. Written here, in the core, so every surface says the
/// same thing — and phrased so the agent's next move is obvious either way.
fn decision_notice(review: &TrustReviewCommand, outcome: TrustRequestState) -> String {
    match outcome {
        TrustRequestState::Granted => format!(
            "Your trust request was approved: `{}` is now trusted in this project and can be started.",
            review.command
        ),
        _ => format!(
            "Your trust request for `{}` was declined. It is still untrusted; do not ask again for the same command without new reason to.",
            review.command
        ),
    }
}

#[cfg(test)]
#[path = "trustrequest_tests.rs"]
mod tests;
