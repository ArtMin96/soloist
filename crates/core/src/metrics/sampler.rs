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

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::supervision::{run_blocking, supervise};
use crate::supervisor::Supervisor;

use super::MetricsProbe;

/// How often each live process group is sampled. One second keeps the per-process event
/// rate at ~1 Hz — comfortably within the UI's ~2 Hz coalescing budget — without polling
/// the OS more than monitoring needs.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

/// How many unchanged samples a steady reading is suppressed before it is re-published as a
/// heartbeat. Emit-on-change stops a steady process (an idle server) from re-sending an identical
/// reading every second, but the UI holds no snapshot to seed from — it accrues from the live
/// stream — so a subscriber that mounts or reloads after the reading last moved would otherwise
/// show a blank reading indefinitely. Re-publishing every `HEARTBEAT_SAMPLES` bounds that gap
/// (~10 s at the 1 Hz sample rate) while still dropping the vast majority of redundant ticks.
const HEARTBEAT_SAMPLES: u32 = 10;

/// The last reading published for a process (CPU bit pattern + RSS bytes) and how many unchanged
/// samples have been suppressed since — the counter drives the [`HEARTBEAT_SAMPLES`] re-publish.
struct Published {
    reading: (u32, u64),
    suppressed: u32,
}

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

    /// The sampling loop itself: tick, read the live groups' metrics, publish a tick for each
    /// group whose reading changed. Ends when the supervisor has been dropped.
    async fn sample_loop(self) {
        // The last reading published per process and how many unchanged samples have been
        // suppressed since, so a steady process — an idle server holding a constant reading — is
        // re-emitted only when its reading moves or a heartbeat falls due (see HEARTBEAT_SAMPLES),
        // not every interval. Bounded by the live set (entries for processes no longer live are
        // dropped each tick). CPU is compared by bit pattern for an exact identity check with no
        // float equality.
        let mut last: HashMap<ProcessId, Published> = HashMap::new();
        loop {
            self.clock.sleep(SAMPLE_INTERVAL).await;
            let Some(supervisor) = self.supervisor.upgrade() else {
                return;
            };
            let targets = supervisor.live_groups();
            // Drop the strong reference before the OS read, so the loop never keeps the
            // supervisor (and the app) alive across a sample.
            drop(supervisor);
            // Forget processes that are no longer live so the cache tracks only the live set.
            last.retain(|id, _| targets.iter().any(|(live, _)| live == id));
            if targets.is_empty() {
                continue;
            }
            let pgids: Vec<i32> = targets.iter().map(|(_, pgid)| *pgid).collect();
            // Read the OS off the runtime so a slow sweep never stalls a worker thread.
            let probe = self.probe.clone();
            let readings = run_blocking(move || probe.sample(&pgids)).await;
            for (id, pgid) in targets {
                if let Some(metrics) = readings.get(&pgid) {
                    let reading = (metrics.cpu_pct.to_bits(), metrics.rss);
                    // An unchanged reading carries nothing new for the UI, so suppress it — until a
                    // heartbeat falls due, so a late or reloaded subscriber still repopulates.
                    if let Some(published) = last.get_mut(&id) {
                        if published.reading == reading
                            && published.suppressed + 1 < HEARTBEAT_SAMPLES
                        {
                            published.suppressed += 1;
                            continue;
                        }
                    }
                    last.insert(
                        id,
                        Published {
                            reading,
                            suppressed: 0,
                        },
                    );
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
