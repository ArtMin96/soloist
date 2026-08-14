use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;

use crate::agents::{AgentActivity, IdleTracker};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::supervision;
use crate::supervisor::Supervisor;
use crate::sync::lock;

use super::state::AgentMailbox;

const MAX_WAKE_RETRIES: u8 = 8;
const WAKE_RETRY_INTERVAL: Duration = Duration::from_millis(500);

impl AgentMailbox {
    pub(crate) fn wake(&self, recipient: ProcessId, supervisor: &Supervisor) -> bool {
        let Some((envelope, claimed)) = self.claim_wake_envelope(recipient) else {
            return false;
        };
        match supervisor.try_submit_turn(recipient, envelope.into_bytes()) {
            Ok(true) => {
                self.mark_wake_submitted(recipient, &claimed);
                true
            }
            Ok(false) | Err(_) => {
                self.release_wake_claim(recipient);
                false
            }
        }
    }

    fn retryable_recipients(&self) -> Vec<ProcessId> {
        let state = lock(&self.state);
        state
            .pending_wakes
            .iter()
            .filter_map(|(recipient, pending)| {
                (pending.attempts < MAX_WAKE_RETRIES && !state.wake_in_flight.contains(recipient))
                    .then_some(*recipient)
            })
            .collect()
    }

    fn record_wake_failure(&self, recipient: ProcessId) {
        let mut state = lock(&self.state);
        if let Some(pending) = state.pending_wakes.get_mut(&recipient) {
            pending.attempts = pending.attempts.saturating_add(1);
        }
    }
}

/// Delivers queued mailbox wake envelopes once an agent's idle classifier says its CLI is ready
/// for another turn — or, for an agent the classifier never types at all, once that wake has
/// waited out [`MAX_WAKE_WAIT`], so a provider that emits no evidence cannot leave its messages
/// queued for the whole run.
///
/// [`MAX_WAKE_WAIT`]: super::vocabulary::MAX_WAKE_WAIT
#[derive(Clone)]
pub struct AgentMailboxReactor {
    mailbox: Arc<AgentMailbox>,
    bus: EventBus,
    supervisor: Weak<Supervisor>,
    idle: Arc<IdleTracker>,
    clock: Arc<dyn Clock>,
}

impl AgentMailboxReactor {
    pub(crate) fn new(
        mailbox: Arc<AgentMailbox>,
        bus: EventBus,
        supervisor: Weak<Supervisor>,
        idle: Arc<IdleTracker>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            mailbox,
            bus,
            supervisor,
            idle,
            clock,
        }
    }

    pub async fn run(self) {
        let clock = self.clock.clone();
        supervision::supervise(clock, move || self.clone().run_loop()).await;
    }

    async fn run_loop(self) {
        let mut events = self.bus.subscribe();
        loop {
            tokio::select! {
            event = events.recv() => match event {
                Ok(DomainEvent::AgentActivityChanged {
                    id,
                    state: AgentActivity::Idle,
                }) => {
                    self.reconcile_one(id);
                }
                Ok(DomainEvent::AgentActivityChanged { id, .. }) => {
                    self.mailbox.observe_non_idle(id);
                }
                Ok(DomainEvent::ProcessRemoved { id }) => self.mailbox.remove_process(id),
                Err(RecvError::Lagged(_)) => self.reconcile_snapshot(),
                Ok(_) => {}
                Err(RecvError::Closed) => return,
            },
            () = self.clock.sleep(WAKE_RETRY_INTERVAL) => {
                self.reconcile_snapshot();
                self.reconcile_backstops();
            }
            }
        }
    }

    fn reconcile_snapshot(&self) {
        let retryable = self.mailbox.retryable_recipients();
        for (recipient, activity) in self.idle.activity_snapshot() {
            if activity == AgentActivity::Idle {
                if retryable.contains(&recipient) {
                    self.reconcile_one(recipient);
                }
            } else {
                self.mailbox.observe_non_idle(recipient);
            }
        }
    }

    /// Delivers the wakes that have waited out [`MAX_WAKE_WAIT`] for an idle classification that
    /// never came. Scoped to a recipient the classifier has never typed: an agent reported busy has
    /// real evidence behind that report, and interrupting it is what the evidence gate exists to
    /// prevent, so it keeps waiting for the idle-gated path. A delivered wake takes its pending
    /// entry with it, which is what makes this one delivery rather than a repeating poke; a wake
    /// the recipient refuses keeps its place and is retried until [`MAX_WAKE_RETRIES`] is spent.
    fn reconcile_backstops(&self) {
        let retryable = self.mailbox.retryable_recipients();
        for recipient in self.mailbox.backstop_candidates() {
            if retryable.contains(&recipient) && self.idle.activity(recipient).is_none() {
                self.reconcile_one(recipient);
            }
        }
    }

    fn reconcile_one(&self, recipient: ProcessId) {
        let Some(supervisor) = self.supervisor.upgrade() else {
            return;
        };
        if !self.mailbox.wake(recipient, &supervisor) {
            self.mailbox.record_wake_failure(recipient);
        }
    }
}

#[cfg(test)]
#[path = "reactor_tests.rs"]
mod tests;
