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
            .wake_attempts
            .iter()
            .filter_map(|(recipient, attempts)| {
                (*attempts < MAX_WAKE_RETRIES && !state.wake_in_flight.contains(recipient))
                    .then_some(*recipient)
            })
            .collect()
    }

    fn record_wake_failure(&self, recipient: ProcessId) {
        let mut state = lock(&self.state);
        if let Some(attempts) = state.wake_attempts.get_mut(&recipient) {
            *attempts = attempts.saturating_add(1);
        }
    }
}

/// Delivers queued mailbox wake envelopes only after an agent's idle classifier says its CLI is
/// ready for another turn.
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
            () = self.clock.sleep(WAKE_RETRY_INTERVAL) => self.reconcile_snapshot(),
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

    fn reconcile_one(&self, recipient: ProcessId) {
        let Some(supervisor) = self.supervisor.upgrade() else {
            return;
        };
        if !self.mailbox.wake(recipient, &supervisor) {
            self.mailbox.record_wake_failure(recipient);
        }
    }
}
