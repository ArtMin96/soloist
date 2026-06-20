//! The monitoring read-model surface: the thin accessors the C5 samplers reach C2 through.
//!
//! The metrics and port-discovery samplers and the readiness wait live in the monitoring
//! domain; they touch the process registry only through these methods, so C2 stays the
//! single owner of the [`crate::process::ProcessView`] while C5 drives the sampling. Each
//! mutation is guarded by the process group it was taken against, so a reading that lands
//! after the group ended is dropped rather than resurrecting state on a resting process.

use crate::events::DomainEvent;
use crate::ids::ProcessId;

use super::Supervisor;

impl Supervisor {
    /// Every running process with a live OS process group, as `(id, leader pgid)`. The
    /// monitoring samplers read this each tick to know what to probe; the supervisor stays
    /// the single owner of which processes are live.
    pub fn live_groups(&self) -> Vec<(ProcessId, i32)> {
        self.registry.live_groups()
    }

    /// The leader pgid of a running process's group, if it has one — what a port-readiness
    /// wait probes. `None` for a resting process.
    pub fn pgid_of(&self, id: ProcessId) -> Option<i32> {
        self.registry.pgid_of(id)
    }

    /// Records a process's freshly discovered listening ports, scoped to the `pgid` they
    /// were scanned against, and returns whether the set changed. The single mutation point
    /// for the port read model — a reading for a group that has since ended is dropped.
    pub fn record_ports(&self, id: ProcessId, pgid: i32, ports: Vec<u16>) -> bool {
        self.registry.set_ports(id, pgid, ports)
    }

    /// Records a process's readiness against the `pgid` it is being waited on and announces
    /// a real change as [`DomainEvent::ReadyStateChanged`]. The single mutation point for
    /// the readiness read model — an update for a group that has ended is dropped; clearing
    /// the gate on stop happens in the registry and is silent.
    pub fn set_ready(&self, id: ProcessId, pgid: i32, ready: bool) {
        if self.registry.set_ready(id, pgid, ready) {
            self.bus
                .publish(DomainEvent::ReadyStateChanged { id, ready });
        }
    }
}
