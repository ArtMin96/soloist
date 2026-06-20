//! The sampling policy: a self-supervised, [`Clock`]-driven loop that publishes a metrics
//! tick per live process group each interval.
//!
//! The *timing* is core policy (mock-clock testable); the OS *read* is the [`MetricsProbe`]
//! adapter's. The sampler holds a [`Weak`] reference to the supervisor, so it ends when the
//! app shuts down (the facade drops) rather than keeping it alive — start it once from the
//! composition root.
//!
//! "Self-supervised" means the sampling loop runs inside a panic-isolation boundary: if it
//! dies, the sampler backs off and restarts it, so a transient probe fault never silently
//! stops monitoring while the rest of the app runs on.

use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::events::{DomainEvent, EventBus};
use crate::ports::Clock;
use crate::supervision::{run_blocking, supervise};
use crate::supervisor::Supervisor;

use super::MetricsProbe;

/// How often each live process group is sampled. One second keeps the per-process event
/// rate at ~1 Hz — comfortably within the UI's ~2 Hz coalescing budget — without polling
/// the OS more than monitoring needs.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

/// Samples live process groups on an interval and publishes their CPU/memory readings.
/// Cloneable so the supervising [`MetricsSampler::run`] can hand a fresh copy to each
/// restart of the inner loop; all clones share the same ports and event bus.
#[derive(Clone)]
pub struct MetricsSampler {
    clock: Arc<dyn Clock>,
    probe: Arc<dyn MetricsProbe>,
    bus: EventBus,
    supervisor: Weak<Supervisor>,
}

impl MetricsSampler {
    /// Builds a sampler over the metrics probe, clock, and event bus, watching the given
    /// supervisor weakly (so it never keeps the app alive).
    pub fn new(
        clock: Arc<dyn Clock>,
        probe: Arc<dyn MetricsProbe>,
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

    /// Runs the sampler until the supervisor is dropped, supervising the inner sampling
    /// loop so a panicking sample is isolated and restarted (see [`supervise`]). Returned
    /// for the composition root to spawn once on its runtime.
    pub async fn run(self) {
        let clock = self.clock.clone();
        supervise(clock, move || self.clone().sample_loop()).await;
    }

    /// The sampling loop itself: tick, read the live groups' metrics, publish a tick each.
    /// Ends when the supervisor has been dropped.
    async fn sample_loop(self) {
        loop {
            self.clock.sleep(SAMPLE_INTERVAL).await;
            let Some(supervisor) = self.supervisor.upgrade() else {
                return;
            };
            let targets = supervisor.live_groups();
            // Drop the strong reference before the OS read, so the loop never keeps the
            // supervisor (and the app) alive across a sample.
            drop(supervisor);
            if targets.is_empty() {
                continue;
            }
            let pgids: Vec<i32> = targets.iter().map(|(_, pgid)| *pgid).collect();
            // Read the OS off the runtime so a slow sweep never stalls a worker thread.
            let probe = self.probe.clone();
            let readings = run_blocking(move || probe.sample(&pgids)).await;
            for (id, pgid) in targets {
                if let Some(metrics) = readings.get(&pgid) {
                    self.bus.publish(DomainEvent::MetricsTick {
                        id,
                        cpu_pct: metrics.cpu_pct,
                        rss: metrics.rss,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "sampler_tests.rs"]
mod tests;
