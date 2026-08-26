//! What a session-scoped caller may do about trust: ask for a command variant to be approved, and
//! read back what the user decided.
//!
//! Asking is the whole surface here. **Approving is not** — that lives on [`Facade`] alone,
//! because a session reaching the core over MCP is another agent, not the person at the keyboard,
//! and a caller that could grant its own request would have no gate at all. The compile-fail
//! probes on [`ScopedFacade`](super::scoped::ScopedFacade) keep it that way.
//!
//! Project and requester come from the authenticated session, never from an argument, so a
//! requester can neither reach another project nor claim to be another process.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::scoped::ScopedFacade;
use super::scoped_process::checked_variant;
use crate::config::InvalidCommand;
use crate::ids::TrustRequestId;
use crate::ports::StoreError;
use crate::trust::TrustRequestSubmission;
use crate::trustrequest::{TrustRequestCapacityError, TrustRequestOutcome, TrustRequestState};

/// What a scoped caller asks the user to approve.
///
/// `working_dir` is the caller's value **as written**. The trust variant digests the raw value, so
/// resolving it against the project root before hashing would show the user one command and
/// authorize a different variant — the failure this whole surface exists to prevent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandTrustRequest {
    /// The command line to be approved.
    pub command: String,
    /// Where it would run, relative to the project root; `None` is the root itself.
    pub working_dir: Option<PathBuf>,
    /// Environment overrides that are part of what is approved.
    pub env: BTreeMap<String, String>,
    /// The display name to show the command under; `None` names it after the command's first word.
    pub label: Option<String>,
    /// Why the caller needs it, in its own words. Required, because it is the only thing standing
    /// between the user and a prompt they approve without reading.
    pub reason: String,
}

/// Why a trust request was refused.
#[derive(Debug, thiserror::Error)]
pub enum RequestTrustError {
    /// The session has no project in scope to request within.
    #[error("no project is in scope; select one first")]
    NoProjectScope,
    /// The session is not bound to a process, so there is nobody to attribute the request to and
    /// nobody for a decision to reach.
    #[error("not bound to a process; bind a session before requesting trust")]
    NoBoundProcess,
    /// The command line, or the name it would be shown under, is not admissible.
    #[error(transparent)]
    InvalidCommand(#[from] InvalidCommand),
    /// A ceiling refused the request without dropping one already awaiting a decision.
    #[error(transparent)]
    Capacity(#[from] TrustRequestCapacityError),
    /// No request under that id is awaiting a decision in this project, and none was recently
    /// resolved there.
    #[error("no such trust request")]
    UnknownRequest,
    /// A durable read failed while checking existing trust — refused, never assumed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ScopedFacade<'_> {
    /// Asks the user to trust a command variant the caller wants to run, returning the request to
    /// poll — or an immediate grant when the variant was already trusted and there was no decision
    /// left to make.
    ///
    /// Recording a request is a **success**, not a refusal: the caller asked and the asking landed.
    /// What the user then decides is read back through
    /// [`trust_request_status`](Self::trust_request_status), which is the channel every requester
    /// has. An agent additionally receives the decision in its mailbox, best-effort.
    ///
    /// Deliberately separate from the action it unblocks. `spawn_process` and `start_process` stay
    /// pure gated actions with one unambiguous refusal each, and the reason — the only thing that
    /// makes the prompt reviewable — stays a required argument rather than an optional extra on
    /// one branch of another tool.
    pub fn request_command_trust(
        &self,
        request: CommandTrustRequest,
    ) -> Result<TrustRequestOutcome, RequestTrustError> {
        let project = self
            .inner
            .effective_project(self.session)
            .ok_or(RequestTrustError::NoProjectScope)?;
        let requested_by = self
            .inner
            .identity
            .origin(self.session)
            .process()
            .ok_or(RequestTrustError::NoBoundProcess)?;
        let (spec, name) = checked_variant(
            request.command,
            request.working_dir,
            request.env,
            request.label,
        )?;
        // A store failure fails closed, matching the start gate: a variant that cannot be verified
        // is not trusted, so the user is asked rather than told it is already fine.
        if self.inner.trust.is_trusted(project, &spec)? {
            return Ok(TrustRequestOutcome {
                request_id: None,
                state: TrustRequestState::Granted,
            });
        }
        let requested_by_label = self
            .inner
            .process_view(requested_by)
            .map_or_else(|| requested_by.to_string(), |view| view.label);
        let id = self.inner.trust_requests.record(TrustRequestSubmission {
            project,
            requested_by,
            requested_by_label,
            name,
            spec,
            reason: request.reason,
        })?;
        Ok(TrustRequestOutcome {
            request_id: Some(id),
            state: TrustRequestState::Pending,
        })
    }

    /// What the user decided about a request in the caller's own project.
    ///
    /// The authoritative channel, and the only one every requester has: a mailbox notice reaches
    /// agents alone, and reaching an agent is best-effort besides. Scoped like every other read,
    /// so a session cannot poll another project's requests.
    pub fn trust_request_status(
        &self,
        id: TrustRequestId,
    ) -> Result<TrustRequestState, RequestTrustError> {
        let project = self
            .inner
            .effective_project(self.session)
            .ok_or(RequestTrustError::NoProjectScope)?;
        self.inner
            .trust_requests
            .status(project, id)
            .ok_or(RequestTrustError::UnknownRequest)
    }
}

#[cfg(test)]
#[path = "scoped_trust_tests.rs"]
mod tests;
