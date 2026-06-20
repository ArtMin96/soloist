//! The port-discovery policy: a self-supervised, [`Clock`]-driven loop that finds each
//! running group's listening ports and reflects changes on the read model.
//!
//! Mirrors the metrics sampler's shape — it reuses the shared [`supervise`] primitive for
//! panic isolation and holds a [`Weak`] reference to the supervisor so it ends at shutdown.
//! It updates [`crate::process::ProcessView::ports`] through the supervisor and announces a
//! real change with [`DomainEvent::PortsChanged`] (so adapters need no snapshot round-trip).

use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::events::{DomainEvent, EventBus};
use crate::ports::Clock;
use crate::supervision::supervise;
use crate::supervisor::Supervisor;

use super::PortProbe;

/// How often listening ports are rescanned. A server binds its port once at startup and
/// rarely changes it, so this is slower than the metrics interval — keeping the `/proc`
/// reads cheap while still catching a port that appears a moment after the process starts.
const SCAN_INTERVAL: Duration = Duration::from_secs(2);

/// Scans live process groups for listening ports and reflects changes on the read model.
/// Cloneable so [`supervise`] can hand a fresh copy to each restart of the inner loop.
#[derive(Clone)]
pub struct PortScanner {
    clock: Arc<dyn Clock>,
    probe: Arc<dyn PortProbe>,
    bus: EventBus,
    supervisor: Weak<Supervisor>,
}

impl PortScanner {
    /// Builds a scanner over the port probe, clock, and event bus, watching the given
    /// supervisor weakly (so it never keeps the app alive).
    pub fn new(
        clock: Arc<dyn Clock>,
        probe: Arc<dyn PortProbe>,
        bus: EventBus,
        supervisor: Weak<Supervisor>,
    ) -> Self {
        Self {
            clock,
            probe,
            bus,
            supervisor,
        }
    }

    /// Runs the scanner until the supervisor is dropped, supervising the inner loop so a
    /// panicking scan is isolated and restarted (see [`supervise`]). Returned for the
    /// composition root to spawn once on its runtime.
    pub async fn run(self) {
        let clock = self.clock.clone();
        supervise(clock, move || self.clone().scan_loop()).await;
    }

    /// The scan loop: each interval, discover every live group's listening ports and record
    /// the changes. Ends when the supervisor has been dropped.
    async fn scan_loop(self) {
        loop {
            self.clock.sleep(SCAN_INTERVAL).await;
            let Some(supervisor) = self.supervisor.upgrade() else {
                return;
            };
            let targets = supervisor.live_groups();
            if targets.is_empty() {
                continue;
            }
            let pgids: Vec<i32> = targets.iter().map(|(_, pgid)| *pgid).collect();
            let mut discovered = self.probe.listening_ports(&pgids);
            for (id, pgid) in targets {
                let ports = discovered.remove(&pgid).unwrap_or_default();
                // Record through the supervisor (the single mutation point) and announce
                // only a real change, so the read model never churns on an unchanged scan.
                if supervisor.record_ports(id, ports.clone()) {
                    self.bus.publish(DomainEvent::PortsChanged { id, ports });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "scanner_tests.rs"]
mod tests;
