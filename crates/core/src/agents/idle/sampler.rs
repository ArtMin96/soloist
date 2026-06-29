//! The idle sampling policy: a self-supervised, [`Clock`]-driven loop that classifies each
//! tracked agent every interval and publishes a [`DomainEvent::AgentActivityChanged`] when
//! its activity changes.
//!
//! The *timing* is core policy (mock-clock testable); the activity is derived from the
//! terminal signals the supervisor exposes. The sampler holds a [`Weak`] reference to the
//! supervisor, so it ends when the app shuts down (the facade drops) rather than keeping it
//! alive — start it once from the composition root.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::agents::AgentLineage;
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::process::ProcStatus;
use crate::supervision::supervise;
use crate::supervisor::Supervisor;

use super::tracker::IdleTracker;

/// How often each tracked agent is reclassified. One second keeps activity responsive (a few
/// of these makes the idle quiet window) without polling the terminal more than needed.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

/// Reclassifies tracked agents on an interval and publishes their activity transitions.
/// Cloneable so the supervising [`IdleSampler::run`] can hand a fresh copy to each restart of
/// the inner loop; all clones share the same tracker, ports, and event bus.
#[derive(Clone)]
pub struct IdleSampler {
    clock: Arc<dyn Clock>,
    tracker: Arc<IdleTracker>,
    lineage: Arc<AgentLineage>,
    bus: EventBus,
    supervisor: Weak<Supervisor>,
}

impl IdleSampler {
    /// Builds a sampler over the idle tracker, the spawn-lineage tracker, the clock, and the
    /// event bus, watching the given supervisor weakly (so it never keeps the app alive). Each
    /// tick also prunes both C4 per-agent maps to the live registry.
    pub fn new(
        clock: Arc<dyn Clock>,
        tracker: Arc<IdleTracker>,
        lineage: Arc<AgentLineage>,
        bus: EventBus,
        supervisor: Weak<Supervisor>,
    ) -> Self {
        Self {
            clock,
            tracker,
            lineage,
            bus,
            supervisor,
        }
    }

    /// Runs the sampler until the supervisor is dropped, supervising the inner loop so a
    /// panicking sample is isolated and restarted. Returned for the composition root to spawn
    /// once on its runtime.
    pub async fn run(self) {
        let clock = self.clock.clone();
        supervise(clock, move || self.clone().sample_loop()).await;
    }

    /// The sampling loop: tick, read each tracked agent's status and terminal signals,
    /// reclassify, and publish a transition. Ends when the supervisor has been dropped.
    async fn sample_loop(self) {
        loop {
            self.clock.sleep(SAMPLE_INTERVAL).await;
            let Some(supervisor) = self.supervisor.upgrade() else {
                return;
            };
            let tracked = self.tracker.tracked();
            // Until an agent is launched there is nothing to classify or prune, so skip the
            // supervisor snapshot entirely. Drop the strong reference first, so an empty tick
            // never keeps the supervisor — and the app — alive across the sleep.
            if tracked.is_empty() {
                drop(supervisor);
                continue;
            }
            let status_by_id: HashMap<ProcessId, ProcStatus> = supervisor
                .snapshot()
                .into_iter()
                .map(|view| (view.id, view.status))
                .collect();
            // Forget agents that have left the registry, so neither C4 per-agent map outgrows it.
            let live: HashSet<ProcessId> = status_by_id.keys().copied().collect();
            self.tracker.retain_live(&live);
            self.lineage.retain_live(&live);
            for id in tracked {
                match status_by_id.get(&id) {
                    // A running agent is reclassified from its current terminal signals.
                    Some(ProcStatus::Running) => {
                        if let Some(signals) = supervisor.terminal_activity(id) {
                            if let Some(state) = self.tracker.observe(id, &signals) {
                                self.bus
                                    .publish(DomainEvent::AgentActivityChanged { id, state });
                            }
                        }
                    }
                    // Registered but not running: reset so a relaunch re-emits its first
                    // activity. (Departed ids were already pruned above.)
                    Some(_) => self.tracker.reset(id),
                    None => {}
                }
            }
            // Drop the strong reference before the next sleep so the loop never keeps the
            // supervisor (and the app) alive across a tick.
            drop(supervisor);
        }
    }
}

#[cfg(test)]
#[path = "sampler_tests.rs"]
mod tests;
