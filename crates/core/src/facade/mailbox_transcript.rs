//! Recording agent-to-agent exchanges and announcing them by id.
//!
//! The mailbox aggregate holds no bus, so retention and announcement are paired here, where the
//! bus is reachable. Every exchange is recorded **after** its enqueue succeeded and its wake
//! attempt settled the outcome: recording earlier would leave a phantom entry behind a refused
//! send.

use super::scoped::ScopedFacade;
use crate::coordination::AgentMessageDelivery;
use crate::events::DomainEvent;

impl ScopedFacade<'_> {
    /// Retains one exchange in its project transcript and announces it by id.
    pub(super) fn record_and_announce(&self, delivery: &AgentMessageDelivery) {
        self.inner.mailbox.record(delivery);
        self.announce_message(delivery);
    }

    /// Moves an exchange already in the transcript to the outcome `delivery` reached, and
    /// announces it. Updating in place rather than appending keeps one exchange one entry.
    pub(super) fn advance_and_announce(&self, delivery: &AgentMessageDelivery) {
        self.inner.mailbox.record_outcome(
            delivery.message.project,
            delivery.message.id,
            delivery.outcome,
        );
        self.announce_message(delivery);
    }

    fn announce_message(&self, delivery: &AgentMessageDelivery) {
        self.inner.bus.publish(DomainEvent::AgentMessageChanged {
            project: delivery.message.project,
            id: delivery.message.id,
        });
    }
}

#[cfg(test)]
#[path = "mailbox_transcript_tests.rs"]
mod tests;
