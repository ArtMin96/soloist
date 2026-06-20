//! The metrics domain's own port: the OS read it depends on, plus the data it reports.
//!
//! Defined here, in the metrics context, rather than in the shared port layer — a new
//! metric or a new probe shape is a change confined to this domain. The adapter
//! (`crates/sys`, over `sysinfo`) implements [`MetricsProbe`]; the core never reads the OS
//! directly.

use std::collections::HashMap;

/// A point-in-time CPU and memory reading for one managed process group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessMetrics {
    /// Aggregate CPU utilization across the group, as a percentage. **Per-core** (the
    /// `htop` convention): a group saturating one core reads ~100, two cores ~200, so a
    /// busy multi-threaded process is not clipped at 100. Never negative.
    pub cpu_pct: f32,
    /// Aggregate resident set size — physical RAM in use across the group — in bytes.
    /// Excludes swapped-out memory.
    pub rss: u64,
}

/// Reads OS-level CPU and memory for managed process groups.
///
/// CPU% is computed from the delta between two successive samples, so an implementation
/// is **stateful**: it samples every requested group in **one pass** per call (refreshing
/// its OS view once) rather than once per group, and the caller drives the cadence via the
/// [`crate::ports::Clock`]. A group is identified by its leader `pgid` (each process spawns
/// into a fresh group whose leader pid is the pgid); the reading aggregates the leader and
/// its descendants. Best-effort: a group with no live member is omitted from the result,
/// and the probe never blocks or panics the core — a missing reading is a missing entry.
pub trait MetricsProbe: Send + Sync {
    /// Samples each group in `groups` (by leader `pgid`) in one pass, returning a reading
    /// per group that still has a live member; groups with none are omitted.
    fn sample(&self, groups: &[i32]) -> HashMap<i32, ProcessMetrics>;
}

/// A [`MetricsProbe`] that reads nothing — the default until the OS adapter is wired
/// (headless tools, tests that do not exercise sampling). The sampler then emits no ticks.
#[derive(Clone, Copy, Default)]
pub struct NoopMetricsProbe;

impl MetricsProbe for NoopMetricsProbe {
    fn sample(&self, _groups: &[i32]) -> HashMap<i32, ProcessMetrics> {
        HashMap::new()
    }
}
