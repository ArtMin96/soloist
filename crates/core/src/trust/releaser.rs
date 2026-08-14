//! Adapts the pending trust-request set to the supervisor's [`LockReleaser`] port, so a process
//! that closes takes its unanswered requests with it.
//!
//! The deterministic hook is the point. Coordination state that rides
//! [`ProcessRemoved`](crate::events::DomainEvent::ProcessRemoved) through a reactor can lag behind
//! the close and recover by reconciling; a security aggregate must not, because between the close
//! and the reconcile the user would be looking at a prompt asking them to authorize a command on
//! behalf of a process that no longer exists. The supervisor calls this the moment a process
//! reaches a terminal state.

use crate::ids::ProcessId;
use crate::ports::LockReleaser;

use super::requests::TrustRequests;

impl LockReleaser for TrustRequests {
    fn release_all(&self, process: ProcessId) {
        self.withdraw_requests_of(process);
    }
}

#[cfg(test)]
#[path = "releaser_tests.rs"]
mod tests;
