use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;

use crate::agents::{AgentActivity, IdleTracker};
use crate::events::{DomainEvent, EventBus};
use crate::ids::{AgentMessageId, ProcessId, ProjectId};
use crate::ports::Clock;
use crate::supervision;
use crate::supervisor::Supervisor;
use crate::sync::lock;

use super::state::AgentMailbox;

const MAX_WAKE_RETRIES: u8 = 8;
const WAKE_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// What one wake attempt did: whether the envelope reached the recipient, and which already
/// recorded exchanges it moved to [`WakeSubmitted`](crate::coordination::AgentMessageOutcome).
/// The mailbox holds no bus, so the advanced entries travel back to the caller that has one.
pub(crate) struct WakeOutcome {
    pub(crate) submitted: bool,
    pub(crate) advanced: Vec<(ProjectId, AgentMessageId)>,
}

impl WakeOutcome {
    fn refused() -> Self {
        Self {
            submitted: false,
            advanced: Vec::new(),
        }
    }
}

impl AgentMailbox {
    pub(crate) fn wake(&self, recipient: ProcessId, supervisor: &Supervisor) -> WakeOutcome {
        let Some((envelope, claimed)) = self.claim_wake_envelope(recipient) else {
            return WakeOutcome::refused();
        };
        match supervisor.try_submit_turn(recipient, envelope.into_bytes()) {
            Ok(true) => WakeOutcome {
                submitted: true,
                advanced: self.mark_wake_submitted(recipient, &claimed),
            },
            Ok(false) | Err(_) => {
                self.release_wake_claim(recipient);
                WakeOutcome::refused()
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
                Ok(DomainEvent::ProjectRemoved { id }) => self.mailbox.forget_project(id),
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
        let outcome = self.mailbox.wake(recipient, &supervisor);
        if !outcome.submitted {
            self.mailbox.record_wake_failure(recipient);
        }
        // A wake delivered here lands long after the send recorded its exchange, so this is the
        // only place the transcript's move to WakeSubmitted can be announced.
        for (project, id) in outcome.advanced {
            self.bus
                .publish(DomainEvent::AgentMessageChanged { project, id });
        }
    }
}

#[cfg(test)]
#[path = "reactor_tests.rs"]
mod tests;
