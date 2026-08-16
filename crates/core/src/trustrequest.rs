//! The vocabulary of a pending request that the user trust a command variant, and the ceilings
//! that hold it.
//!
//! Shared kernel, not a context, for the reason [`crate::configchange`] states about
//! [`TrustReviewCommand`]: [`crate::events`] carries these types, and the trust context both
//! produces them and publishes those events — so if that context owned them, the event bus and it
//! would import each other. They depend on nothing and live here instead.
//!
//! Only the *types* moved. [`crate::trust`] still holds the aggregate that mints, dedupes, expires
//! and resolves them.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::configchange::TrustReviewCommand;
use crate::ids::{ProcessId, ProjectId, TrustRequestId};

/// Maximum UTF-8 bytes in the reason a requester gives. A reason is one sentence explaining why a
/// command is needed, not a payload, so it is far smaller than an addressed message body — and it
/// is attacker-controlled text a person has to read, which is its own argument for a tight bound.
pub const MAX_TRUST_REQUEST_REASON_BYTES: usize = 4 * 1024;

/// Maximum pending requests held for one project. Far smaller than any message queue because every
/// entry costs a *human decision* rather than memory: a thousand queued approval prompts is itself
/// the denial of service.
pub const MAX_PENDING_TRUST_REQUESTS_PER_PROJECT: usize = 16;

/// Maximum pending requests held across the entire running application.
pub const MAX_PENDING_TRUST_REQUESTS: usize = 64;

/// How long a pending request waits for a decision before it reads back as expired. Longer than a
/// coordination lease, because a person has to notice it; short enough that a prompt nobody
/// answered does not sit overnight attached to a process that has moved on.
pub const TRUST_REQUEST_TTL: Duration = Duration::from_secs(10 * 60);

/// One process's open request that the user trust a command variant.
///
/// `project` and `requested_by` come from the authenticated session, never from the caller, so a
/// requester can neither reach another project nor claim to be another process. `review` carries
/// the exact command line, working directory and environment the user is shown, pinned to the
/// `variant_hash` an approval must re-derive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TrustRequest {
    pub id: TrustRequestId,
    pub project: ProjectId,
    /// The process that asked, for attribution — shown beside its label so the user knows who is
    /// asking rather than only what is being asked for.
    pub requested_by: ProcessId,
    /// That process's display label as it read when the request was made.
    pub requested_by_label: String,
    /// What would run, and the variant an approval authorizes.
    pub review: TrustReviewCommand,
    /// The requester's own words. **Agent-supplied and untrusted**: render it as an attributed
    /// quotation in plain text, never as the application's own prose and never as markup.
    pub reason: String,
    /// The instant past which this request reads back as [`TrustRequestState::Expired`].
    pub expires_unix_millis: u64,
}

/// Where a request stands — the authoritative answer every requester can poll for, whether or not
/// it has a mailbox to be told through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustRequestState {
    /// Recorded and waiting for the user.
    Pending,
    /// The user approved it; the variant is trusted in the project.
    Granted,
    /// The user declined it; nothing was trusted.
    Denied,
    /// Nobody answered within [`TRUST_REQUEST_TTL`]; nothing was trusted.
    Expired,
    /// The requesting process closed before a decision was made, so the request was dropped
    /// rather than left inviting approval on behalf of a process that no longer exists.
    Withdrawn,
}

/// What recording a request produced: the id to poll, or an immediate grant because the variant
/// was already trusted and there was no decision left to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustRequestOutcome {
    /// The request to poll with `trust_request_status`; `None` when the variant was already
    /// trusted, so nothing was recorded.
    pub request_id: Option<TrustRequestId>,
    pub state: TrustRequestState,
}

/// A ceiling that refused a request without dropping one already queued. Refusing rather than
/// evicting is a security property here, not tidiness: making room would let a flood of requests
/// silently displace the one the user was about to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TrustRequestCapacityError {
    #[error("a trust request's reason is limited to {MAX_TRUST_REQUEST_REASON_BYTES} bytes")]
    ReasonTooLarge,
    #[error("this project already has {MAX_PENDING_TRUST_REQUESTS_PER_PROJECT} trust requests awaiting a decision")]
    ProjectQueueFull,
    #[error("{MAX_PENDING_TRUST_REQUESTS} trust requests are already awaiting a decision")]
    GlobalQueueFull,
}
