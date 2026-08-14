//! The pending trust-request aggregate: what a bound process has asked the user to approve, and
//! has not yet been answered.
//!
//! Ephemeral by construction. A request is meaningful only while the process that made it is alive
//! and this run continues; closing the app stops every process, so a request that outlived its run
//! would be an approval prompt for a command nobody is asking for any more, attributable to a dead
//! process. The *grant* an approval writes is durable — the asking is not.
//!
//! Two rules carry the security argument. The whole [`ProcessSpec`] is pinned here, and the
//! [`TrustReviewCommand`] the user is shown is a projection of it, so an approval re-derives the
//! variant it authorizes from the very value that was displayed. And every ceiling **refuses**
//! rather than evicting: making room would let a flood of requests silently displace the one the
//! user was about to read.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::config::ProcessSpec;
use crate::configchange::TrustReviewCommand;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId, TrustRequestId};
use crate::ports::Clock;
use crate::sync::lock;
use crate::trustrequest::{
    TrustRequest, TrustRequestCapacityError, TrustRequestState, MAX_PENDING_TRUST_REQUESTS,
    MAX_PENDING_TRUST_REQUESTS_PER_PROJECT, MAX_TRUST_REQUEST_REASON_BYTES, TRUST_REQUEST_TTL,
};

/// A request still awaiting a decision, with the spec its review was built from. The spec never
/// leaves the aggregate as anything but a copy of what was pinned, which is what lets an approval
/// re-derive the displayed variant instead of trusting the hash it was handed.
#[derive(Clone, Debug)]
pub struct PendingTrustRequest {
    pub request: TrustRequest,
    pub spec: ProcessSpec,
}

/// What a caller supplies to open a request. Project and requester are resolved from the
/// authenticated session by the façade, never taken from the caller's arguments.
pub struct TrustRequestSubmission {
    pub project: ProjectId,
    pub requested_by: ProcessId,
    pub requested_by_label: String,
    /// The display name the command would be filed under, shown beside what it runs.
    pub name: String,
    /// The command variant, with `working_dir` exactly as the caller wrote it — the trust variant
    /// digests the raw value, so resolving it against the project root first would authorize a
    /// different command than the one displayed.
    pub spec: ProcessSpec,
    pub reason: String,
}

#[derive(Default)]
struct RequestState {
    pending: Vec<PendingTrustRequest>,
    /// The outcome of each recently resolved request, so a poll answers authoritatively after the
    /// pending entry is gone. The project stays on the receipt because a poll is scope-checked
    /// like every other scoped read. Bounded like the pending set; a receipt is a notice, not a
    /// decision, so the oldest is dropped when the ring is full.
    resolved: VecDeque<(TrustRequestId, ProjectId, TrustRequestState)>,
}

/// The per-run set of open trust requests.
pub struct TrustRequests {
    state: Mutex<RequestState>,
    clock: Arc<dyn Clock>,
    bus: EventBus,
}

impl TrustRequests {
    /// An empty set that measures its TTL against `clock` and announces every change on `bus`.
    pub fn new(clock: Arc<dyn Clock>, bus: EventBus) -> Self {
        Self {
            state: Mutex::new(RequestState::default()),
            clock,
            bus,
        }
    }

    /// Records `submission` and announces it, or returns the id of an identical request already
    /// awaiting a decision.
    ///
    /// Deduped on `(project, variant_hash)` — the key the durable grant itself uses — so N
    /// processes asking for one command line produce **one** prompt. The requester is recorded for
    /// attribution but is deliberately not part of the key: were it, a group of agents could raise
    /// one prompt each for the identical command.
    pub fn record(
        &self,
        submission: TrustRequestSubmission,
    ) -> Result<TrustRequestId, TrustRequestCapacityError> {
        if submission.reason.len() > MAX_TRUST_REQUEST_REASON_BYTES {
            return Err(TrustRequestCapacityError::ReasonTooLarge);
        }
        let now = self.clock.now_unix_millis();
        let review = TrustReviewCommand::from_spec(&submission.name, &submission.spec);
        let expired;
        let recorded = {
            let mut state = lock(&self.state);
            expired = prune_expired(&mut state, now);
            let duplicate = state
                .pending
                .iter()
                .find(|held| {
                    held.request.project == submission.project
                        && held.request.review.variant_hash == review.variant_hash
                })
                .map(|held| held.request.id);
            match duplicate {
                Some(id) => Ok(Recorded::Existing(id)),
                None if state.pending.len() >= MAX_PENDING_TRUST_REQUESTS => {
                    Err(TrustRequestCapacityError::GlobalQueueFull)
                }
                None if project_count(&state, submission.project)
                    >= MAX_PENDING_TRUST_REQUESTS_PER_PROJECT =>
                {
                    Err(TrustRequestCapacityError::ProjectQueueFull)
                }
                None => {
                    let request = TrustRequest {
                        id: TrustRequestId::next(),
                        project: submission.project,
                        requested_by: submission.requested_by,
                        requested_by_label: submission.requested_by_label,
                        review,
                        reason: submission.reason,
                        expires_unix_millis: now
                            .saturating_add(TRUST_REQUEST_TTL.as_millis() as u64),
                    };
                    state.pending.push(PendingTrustRequest {
                        request: request.clone(),
                        spec: submission.spec,
                    });
                    Ok(Recorded::Fresh(request))
                }
            }
        };
        self.announce_resolved(expired);
        match recorded? {
            Recorded::Existing(id) => Ok(id),
            Recorded::Fresh(request) => {
                let id = request.id;
                self.bus.publish(DomainEvent::TrustRequested {
                    project: request.project,
                    request,
                });
                Ok(id)
            }
        }
    }

    /// Where `id` stands within `project`, or `None` when no request under that id is remembered
    /// there. Scoped like every other read a session-limited caller makes, so polling cannot
    /// observe another project's requests. Expiry is applied on this read rather than by a timer,
    /// so a request past its TTL reads back [`TrustRequestState::Expired`] and frees its slot.
    pub fn status(&self, project: ProjectId, id: TrustRequestId) -> Option<TrustRequestState> {
        let now = self.clock.now_unix_millis();
        let (found, expired) = {
            let mut state = lock(&self.state);
            let expired = prune_expired(&mut state, now);
            let found = state
                .pending
                .iter()
                .any(|held| held.request.id == id && held.request.project == project)
                .then_some(TrustRequestState::Pending)
                .or_else(|| {
                    state
                        .resolved
                        .iter()
                        .find(|(resolved, owner, _)| *resolved == id && *owner == project)
                        .map(|(_, _, outcome)| *outcome)
                });
            (found, expired)
        };
        self.announce_resolved(expired);
        found
    }

    /// Every request in `project` still awaiting a decision, oldest first — what the approval
    /// surface renders.
    pub fn pending(&self, project: ProjectId) -> Vec<TrustRequest> {
        let now = self.clock.now_unix_millis();
        let (open, expired) = {
            let mut state = lock(&self.state);
            let expired = prune_expired(&mut state, now);
            let open = state
                .pending
                .iter()
                .filter(|held| held.request.project == project)
                .map(|held| held.request.clone())
                .collect();
            (open, expired)
        };
        self.announce_resolved(expired);
        open
    }

    /// A copy of the pinned request under `id`, or `None` when it is not awaiting a decision.
    /// Reading does not remove it: a grant that fails part-way leaves the request open rather than
    /// silently discarding the user's decision.
    pub fn peek(&self, id: TrustRequestId) -> Option<PendingTrustRequest> {
        let now = self.clock.now_unix_millis();
        let (found, expired) = {
            let mut state = lock(&self.state);
            let expired = prune_expired(&mut state, now);
            let found = state
                .pending
                .iter()
                .find(|held| held.request.id == id)
                .cloned();
            (found, expired)
        };
        self.announce_resolved(expired);
        found
    }

    /// Removes `id` from the pending set with `outcome`, records the receipt a later poll reads,
    /// and announces the resolution. Returns the request that was resolved, or `None` when it was
    /// no longer pending.
    pub fn resolve(&self, id: TrustRequestId, outcome: TrustRequestState) -> Option<TrustRequest> {
        let resolved = {
            let mut state = lock(&self.state);
            let index = state
                .pending
                .iter()
                .position(|held| held.request.id == id)?;
            let held = state.pending.remove(index);
            record_receipt(&mut state, id, held.request.project, outcome);
            held.request
        };
        self.bus.publish(DomainEvent::TrustRequestResolved {
            project: resolved.project,
            id: resolved.id,
            state: outcome,
        });
        Some(resolved)
    }

    /// Drops every request `process` opened, marking each [`TrustRequestState::Withdrawn`] and
    /// announcing it — so an approval prompt already on screen for a process that has closed goes
    /// away rather than inviting a grant on its behalf.
    pub(super) fn withdraw_requests_of(&self, process: ProcessId) {
        let withdrawn: Vec<_> = {
            let mut state = lock(&self.state);
            let (leaving, staying): (Vec<_>, Vec<_>) = std::mem::take(&mut state.pending)
                .into_iter()
                .partition(|held| held.request.requested_by == process);
            state.pending = staying;
            for held in &leaving {
                record_receipt(
                    &mut state,
                    held.request.id,
                    held.request.project,
                    TrustRequestState::Withdrawn,
                );
            }
            leaving
                .into_iter()
                .map(|held| (held.request.project, held.request.id))
                .collect()
        };
        for (project, id) in withdrawn {
            self.bus.publish(DomainEvent::TrustRequestResolved {
                project,
                id,
                state: TrustRequestState::Withdrawn,
            });
        }
    }

    /// Announces the requests a read found to have aged out, once the aggregate's lock is released.
    fn announce_resolved(&self, expired: Vec<(ProjectId, TrustRequestId)>) {
        for (project, id) in expired {
            self.bus.publish(DomainEvent::TrustRequestResolved {
                project,
                id,
                state: TrustRequestState::Expired,
            });
        }
    }
}

/// Whether a submission opened a fresh request or matched one already awaiting a decision.
enum Recorded {
    Fresh(TrustRequest),
    Existing(TrustRequestId),
}

/// How many of `project`'s requests are awaiting a decision.
fn project_count(state: &RequestState, project: ProjectId) -> usize {
    state
        .pending
        .iter()
        .filter(|held| held.request.project == project)
        .count()
}

/// Moves every request past its expiry out of the pending set, recording each as
/// [`TrustRequestState::Expired`], and reports them so the caller can announce them after
/// unlocking.
fn prune_expired(state: &mut RequestState, now: u64) -> Vec<(ProjectId, TrustRequestId)> {
    let (aged, live): (Vec<_>, Vec<_>) = std::mem::take(&mut state.pending)
        .into_iter()
        .partition(|held| held.request.expires_unix_millis <= now);
    state.pending = live;
    for held in &aged {
        record_receipt(
            state,
            held.request.id,
            held.request.project,
            TrustRequestState::Expired,
        );
    }
    aged.into_iter()
        .map(|held| (held.request.project, held.request.id))
        .collect()
}

/// Files an outcome a later poll reads back, dropping the oldest receipt when the ring is full.
fn record_receipt(
    state: &mut RequestState,
    id: TrustRequestId,
    project: ProjectId,
    outcome: TrustRequestState,
) {
    if state.resolved.len() >= MAX_PENDING_TRUST_REQUESTS {
        state.resolved.pop_front();
    }
    state.resolved.push_back((id, project, outcome));
}

#[cfg(test)]
#[path = "requests_tests.rs"]
mod tests;
