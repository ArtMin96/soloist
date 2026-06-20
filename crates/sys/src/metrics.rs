//! CPU/memory per process group, read from `/proc`: the OS read behind the core's
//! `MetricsProbe`.
//!
//! For each requested group (its leader pid, which is the group's pgid) it resolves the exact
//! process-group membership (`crate::proc`) and aggregates:
//!
//! - **Memory** as summed **PSS** (proportional set size, `/proc/<pid>/smaps_rollup`): shared
//!   pages — a shared interpreter/compiler binary, shared libraries — are counted once across
//!   the group, proportionally, rather than once per process. Summing plain RSS instead would
//!   multiply shared memory by the number of processes (a build of dozens of compiler
//!   processes would read tens of GB it does not actually occupy). Falls back to resident RSS
//!   (`/proc/<pid>/statm`) only where `smaps_rollup` is unavailable.
//! - **CPU** as the group's CPU-time delta since the previous sample, normalised to the whole
//!   machine: 100% means every core is busy, so a value never exceeds 100 (not the per-core
//!   `htop` convention, where a build across many cores reads several hundred percent).
//!   Per-pid tick baselines are kept across calls, so a process appearing or exiting
//!   mid-interval never spikes the reading.

use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use std::time::Instant;

use soloist_core::{MetricsProbe, ProcessMetrics};

use crate::proc::{group_members, read_cpu_ticks};

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;

/// Reads per-group CPU and memory from `/proc`. CPU% is normalised to the whole machine
/// (100% = every core busy); memory is the group's proportional set size.
pub struct ProcMetricsProbe {
    /// Per-pid CPU-tick baselines plus the previous sample instant, so CPU% is a delta over
    /// real elapsed time and a churning membership never spikes it.
    state: Mutex<CpuBaseline>,
}

#[derive(Default)]
struct CpuBaseline {
    at: Option<Instant>,
    ticks_by_pid: HashMap<i32, u64>,
}

impl ProcMetricsProbe {
    /// A probe with no CPU baseline; the first sample primes it (and reads 0% CPU).
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CpuBaseline::default()),
        }
    }
}

impl Default for ProcMetricsProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsProbe for ProcMetricsProbe {
    fn sample(&self, groups: &[i32]) -> HashMap<i32, ProcessMetrics> {
        if groups.is_empty() {
            return HashMap::new();
        }
        let members = group_members();
        let now = Instant::now();
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as f64;
        let clk_tck = clock_ticks_per_sec();

        // Never let a poisoned lock stop monitoring: recover the guard and read on.
        let mut baseline = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let elapsed = baseline
            .at
            .map(|prev| now.duration_since(prev).as_secs_f64())
            .unwrap_or(0.0);

        // The new per-pid baseline, holding only the processes seen this sweep so the map
        // stays bounded to the currently-live members of the requested groups.
        let mut current_ticks: HashMap<i32, u64> = HashMap::new();
        let mut readings = HashMap::new();
        for &pgid in groups {
            // A group with no live member has exited — report nothing for it.
            let Some(pids) = members.get(&pgid) else {
                continue;
            };
            let mut delta_ticks = 0_u64;
            let mut pss_bytes = 0_u64;
            for &pid in pids {
                if let Some(ticks) = read_cpu_ticks(pid) {
                    // A pid with no prior baseline contributes nothing this round (it gets a
                    // baseline now), so a freshly-spawned child never reads as a CPU spike.
                    let prev = baseline.ticks_by_pid.get(&pid).copied().unwrap_or(ticks);
                    delta_ticks += ticks.saturating_sub(prev);
                    current_ticks.insert(pid, ticks);
                }
                pss_bytes += process_memory_bytes(pid);
            }
            readings.insert(
                pgid,
                ProcessMetrics {
                    cpu_pct: cpu_percent(delta_ticks, clk_tck, elapsed, cores),
                    rss: pss_bytes,
                },
            );
        }

        baseline.ticks_by_pid = current_ticks;
        baseline.at = Some(now);
        readings
    }
}

/// The whole-machine CPU percentage from a group's tick delta: `ticks / clk_tck` seconds of
/// CPU over `elapsed` seconds of wall time, divided by the core count so full use of every
/// core reads 100%. Zero before the first interval (no baseline yet).
fn cpu_percent(delta_ticks: u64, clk_tck: f64, elapsed_secs: f64, cores: f64) -> f32 {
    if elapsed_secs <= 0.0 || clk_tck <= 0.0 || cores <= 0.0 {
        return 0.0;
    }
    let cpu_secs = delta_ticks as f64 / clk_tck;
    ((cpu_secs / elapsed_secs / cores) * 100.0) as f32
}

/// A process's memory in bytes: its proportional set size where the kernel exposes it
/// (`smaps_rollup`), else its resident set (`statm`). Zero if neither is readable (the
/// process exited mid-sweep), which simply omits it from the group total.
fn process_memory_bytes(pid: i32) -> u64 {
    read_pss_bytes(pid)
        .or_else(|| read_rss_bytes(pid))
        .unwrap_or(0)
}

/// The proportional set size (bytes) from `/proc/<pid>/smaps_rollup`, which counts shared
/// pages once across all sharers — the honest per-process share of memory.
fn read_pss_bytes(pid: i32) -> Option<u64> {
    let rollup = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
    rollup
        .lines()
        .find_map(|line| line.strip_prefix("Pss:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kib| kib.parse::<u64>().ok())
        .map(|kib| kib * 1024)
}

/// The resident set size (bytes) from `/proc/<pid>/statm` (resident pages × page size) — the
/// fallback when `smaps_rollup` is absent. Counts shared pages in each sharer, so summing it
/// across a group overstates shared memory; PSS is preferred.
fn read_rss_bytes(pid: i32) -> Option<u64> {
    let statm = fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    let resident_pages = statm.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(resident_pages * page_size())
}

/// Clock ticks per second (`USER_HZ`), for converting `/proc` CPU times to seconds.
fn clock_ticks_per_sec() -> f64 {
    // SAFETY: `sysconf` is a pure query of a system constant with no preconditions.
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if hz > 0 {
        hz as f64
    } else {
        100.0
    }
}

/// The memory page size in bytes, for converting `statm` page counts to bytes.
fn page_size() -> u64 {
    // SAFETY: `sysconf` is a pure query of a system constant with no preconditions.
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if size > 0 {
        size as u64
    } else {
        4096
    }
}
