//! The typed domain event bus.
//!
//! Contexts publish [`DomainEvent`]s — deltas over a snapshot — onto a bounded
//! `tokio::sync::broadcast` channel; adapters subscribe and project them into their
//! own read models. The contract is **snapshot-then-deltas**: an adapter first reads
//! a full snapshot (e.g. [`crate::facade::Facade::snapshot`]), then applies events.
//! If a slow subscriber lags and the channel drops messages, `recv` reports
//! `Lagged`; the adapter recovers by re-reading the snapshot rather than trusting a
//! gap-filled stream. The channel is bounded so a stalled subscriber can never grow
//! memory without limit.

use serde::Serialize;
use tokio::sync::broadcast;

use crate::ids::ProcessId;
use crate::process::{ProcStatus, ProcessKind};

/// A change in domain state, serialized to adapters verbatim. `#[serde(tag = "type")]`
/// gives each variant a discriminator field so a JS/TS consumer can switch on it.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    /// A new process entered the registry (initial status included).
    ProcessSpawned {
        id: ProcessId,
        kind: ProcessKind,
        label: String,
        status: ProcStatus,
    },
    /// A process moved between lifecycle states.
    ProcessStatusChanged {
        id: ProcessId,
        from: ProcStatus,
        to: ProcStatus,
    },
    /// A process left the registry.
    ProcessRemoved { id: ProcessId },
}

/// The outbound event port: anything the core publishes domain events through.
///
/// Realized in the walking skeleton by [`EventBus`]. Defined as a trait so an
/// adapter that needs a different fan-out shape (e.g. an MCP push sink) can provide
/// its own implementation without the core depending on it.
pub trait EventSink: Send + Sync {
    /// Publishes an event. Best-effort: a sink with no live receivers drops it.
    fn emit(&self, event: DomainEvent);
}

/// A bounded broadcast bus carrying [`DomainEvent`]s from the core to all adapters.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
}

impl EventBus {
    /// Creates a bus whose channel buffers at most `capacity` undelivered events per
    /// subscriber before the slowest subscriber starts observing `Lagged`.
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribes a new receiver. Adapters pair this with a fresh snapshot read.
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }

    /// Publishes an event; dropped silently when no subscribers are attached.
    pub fn publish(&self, event: DomainEvent) {
        let _ = self.tx.send(event);
    }
}

impl EventSink for EventBus {
    fn emit(&self, event: DomainEvent) {
        self.publish(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ProcessId;

    #[tokio::test]
    async fn published_events_reach_a_subscriber() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();
        let id = ProcessId::next();
        bus.publish(DomainEvent::ProcessRemoved { id });
        match rx.recv().await {
            Ok(DomainEvent::ProcessRemoved { id: got }) => assert_eq!(got, id),
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
