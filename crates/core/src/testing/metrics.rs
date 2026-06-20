//! A [`MetricsProbe`] fake for sampler tests: it returns a fixed reading for every
//! requested group without touching the OS, and can be told to panic on its first
//! sample so the metrics sampler's self-supervision (auto-restart on panic) is testable.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::metrics::{MetricsProbe, ProcessMetrics};

/// An in-memory [`MetricsProbe`]: it reports the same reading for each group it is asked
/// about and counts its calls, so a test can assert ticks fired. With [`FakeMetricsProbe::panic_once`]
/// set, the first sample panics — driving the sampler's panic-isolation boundary — and
/// every later sample returns the fixed reading, proving it resumed.
#[derive(Clone)]
pub struct FakeMetricsProbe {
    reading: ProcessMetrics,
    calls: Arc<AtomicUsize>,
    panic_pending: Arc<AtomicBool>,
}

impl FakeMetricsProbe {
    /// A probe that reports `cpu_pct`/`rss` for every requested group.
    pub fn returning(cpu_pct: f32, rss: u64) -> Self {
        Self {
            reading: ProcessMetrics { cpu_pct, rss },
            calls: Arc::new(AtomicUsize::new(0)),
            panic_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Arms the probe to panic on its next (first) sample, then behave normally — used to
    /// prove the sampler restarts itself after a sampling task dies.
    pub fn panic_once(self) -> Self {
        self.panic_pending.store(true, Ordering::SeqCst);
        self
    }

    /// How many times [`MetricsProbe::sample`] has been called (panicking calls included).
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl MetricsProbe for FakeMetricsProbe {
    fn sample(&self, groups: &[i32]) -> HashMap<i32, ProcessMetrics> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.panic_pending.swap(false, Ordering::SeqCst) {
            #[allow(clippy::panic)]
            {
                panic!("fake metrics probe panicked");
            }
        }
        groups.iter().map(|&pgid| (pgid, self.reading)).collect()
    }
}
