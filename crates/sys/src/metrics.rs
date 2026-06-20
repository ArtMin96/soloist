//! The CPU/memory probe over `sysinfo`: the OS read behind the core's `MetricsProbe`.
//!
//! A managed process spawns into a fresh process group whose leader pid is the group's
//! pgid; this probe aggregates the leader and its descendants (its process subtree, by
//! parent) into one reading, so a dev server whose work happens in child processes reports
//! real usage. The subtree is an approximation of the OS process group — a descendant that
//! reparents to init (a double-fork) escapes it — because `sysinfo` does not expose the
//! process group; it is good enough for the aggregate CPU/RSS figure. It holds a persistent
//! `sysinfo::System` because CPU% is a delta between successive refreshes — the caller (the
//! core sampler) drives the cadence; this refreshes once per call across every requested
//! group.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use soloist_core::{MetricsProbe, ProcessMetrics};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Reads per-group CPU and memory from the OS via `sysinfo`. CPU% is **per-core** (the
/// `htop` convention): a group saturating two cores reads ~200, not clipped at 100.
pub struct SysinfoMetricsProbe {
    /// Held across calls so CPU% can be computed from the diff between refreshes.
    system: Mutex<System>,
}

impl SysinfoMetricsProbe {
    /// A probe with an empty process view; the first sample primes the CPU delta.
    pub fn new() -> Self {
        Self {
            system: Mutex::new(System::new()),
        }
    }
}

impl Default for SysinfoMetricsProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsProbe for SysinfoMetricsProbe {
    fn sample(&self, groups: &[i32]) -> HashMap<i32, ProcessMetrics> {
        if groups.is_empty() {
            return HashMap::new();
        }
        // Never let a poisoned lock stop monitoring: recover the guard and read on.
        let mut system = self
            .system
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );

        // Build the parent → children map once from the refreshed snapshot, so each group's
        // subtree walk is cheap (no per-process syscall).
        let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
        for (pid, process) in system.processes() {
            if let Some(parent) = process.parent() {
                children.entry(parent).or_default().push(*pid);
            }
        }

        groups
            .iter()
            .filter_map(|&pgid| {
                aggregate_subtree(&system, &children, Pid::from_u32(pgid as u32))
                    .map(|metrics| (pgid, metrics))
            })
            .collect()
    }
}

/// Sums CPU% and resident memory across the subtree rooted at `leader`. Returns `None` if
/// the leader is no longer a live process (the group has exited), so the caller omits it.
fn aggregate_subtree(
    system: &System,
    children: &HashMap<Pid, Vec<Pid>>,
    leader: Pid,
) -> Option<ProcessMetrics> {
    // A group with no live leader is gone — report nothing for it.
    system.process(leader)?;

    let mut cpu_pct = 0.0_f32;
    let mut rss = 0_u64;
    let mut seen = HashSet::new();
    let mut stack = vec![leader];
    while let Some(pid) = stack.pop() {
        // Guard against a malformed adjacency forming a cycle.
        if !seen.insert(pid) {
            continue;
        }
        if let Some(process) = system.process(pid) {
            cpu_pct += process.cpu_usage();
            rss += process.memory();
        }
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids.iter().copied());
        }
    }
    Some(ProcessMetrics { cpu_pct, rss })
}
