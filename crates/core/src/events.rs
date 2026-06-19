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

use std::path::PathBuf;

use serde::Serialize;
use tokio::sync::broadcast;

use crate::config::{ConfigSync, TrustReviewCommand};
use crate::ids::{ProcessId, ProjectId};
use crate::orphans::OrphanInfo;
use crate::process::{ProcStatus, ProcessKind};

/// A change in domain state, serialized to adapters verbatim. `#[serde(tag = "type")]`
/// gives each variant a discriminator field so a JS/TS consumer can switch on it.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    /// A new process entered the registry (initial status included). `requires_trust`
    /// is true for a trust-gated command whose variant is not yet trusted — the UI
    /// blocks its start and offers a trust affordance.
    ProcessSpawned {
        id: ProcessId,
        project: ProjectId,
        kind: ProcessKind,
        label: String,
        status: ProcStatus,
        requires_trust: bool,
    },
    /// A process moved between lifecycle states. `exit_code` is set on a terminal
    /// transition driven by the child exiting on its own (`None` when terminated by a
    /// signal or for non-terminal transitions).
    ProcessStatusChanged {
        id: ProcessId,
        from: ProcStatus,
        to: ProcStatus,
        exit_code: Option<i32>,
    },
    /// A process left the registry.
    ProcessRemoved { id: ProcessId },
    /// A project was opened: its durable id, resolved display name, and root. Lets
    /// adapters add the project to their read model live, without re-querying the
    /// snapshot. Carries the same display identity as [`crate::projects::ProjectView`].
    ProjectOpened {
        id: ProjectId,
        name: String,
        root: PathBuf,
    },
    /// A project's `solo.yml` changed on disk. Carries the add/update/remove/rename
    /// diff, whether any added/updated command now needs (re-)trust, and the detail of
    /// each command awaiting trust (so the review dialog can show what will run). Sync
    /// never starts a process — this event only informs adapters of the change.
    ConfigChanged {
        project: ProjectId,
        diff: ConfigSync,
        requires_trust: bool,
        commands: Vec<TrustReviewCommand>,
    },
    /// A process set its terminal title via an OSC sequence. Drives window/tab titles
    /// and feeds the agent idle heuristics that watch title stability.
    TerminalTitleChanged { id: ProcessId, title: String },
    /// A process rang the terminal bell (`BEL`). Drives attention notifications.
    TerminalBell { id: ProcessId },
    /// Reconciliation found leftover process groups from a previous run that match no
    /// known command, awaiting a user Kill / Kill All / Leave decision surfaced by the
    /// UI. The core only reports them; it neither kills nor keeps them on its own.
    OrphansFound { orphans: Vec<OrphanInfo> },
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
