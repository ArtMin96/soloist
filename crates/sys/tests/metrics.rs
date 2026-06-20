//! Integration check against the real `/proc` metrics adapter: it reads a live process group
//! and omits a dead one, and its memory figure is the group's proportional set size — not the
//! sum of per-process RSS that would multiply shared memory into implausible totals. The CPU
//! normalisation math is unit-tested in the crate (`metrics_tests.rs`).

use std::os::unix::process::CommandExt;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use soloist_core::MetricsProbe;
use soloist_sys::ProcMetricsProbe;

/// A pid that will not exist on a normal Linux system (well above `pid_max`).
const ABSENT_PID: i32 = 999_999_999;

/// Spawns a child as its own process-group leader (so its pid is its pgid, exactly as the
/// supervisor spawns managed processes) running a long-lived, low-CPU command.
fn spawn_group_leader() -> std::process::Child {
    let mut command = Command::new("sleep");
    command.arg("30");
    // SAFETY: `setpgid` is async-signal-safe and the only call in the pre-exec hook.
    unsafe {
        command.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }
    command.spawn().expect("spawn a child process group")
}

#[test]
fn samples_a_live_group_with_a_plausible_memory_figure() {
    let probe = ProcMetricsProbe::new();
    let mut child = spawn_group_leader();
    let pgid = child.id() as i32;
    // Give the child a moment to be visible in /proc, then sample twice for a CPU delta.
    sleep(Duration::from_millis(50));
    let _ = probe.sample(&[pgid]);
    sleep(Duration::from_millis(100));
    let readings = probe.sample(&[pgid, ABSENT_PID]);

    let group = readings
        .get(&pgid)
        .expect("the live child group is sampled");
    assert!(group.rss > 0, "a live process uses some memory");
    // `sleep` occupies a few megabytes; the regression this guards is summed-RSS
    // double-counting, which inflated a real group to multiple gigabytes.
    assert!(
        group.rss < 256 * 1024 * 1024,
        "a trivial process group's PSS is well under 256 MiB, got {} bytes",
        group.rss,
    );
    assert!(
        group.cpu_pct.is_finite() && group.cpu_pct >= 0.0,
        "cpu% is a sane non-negative number, got {}",
        group.cpu_pct,
    );
    assert!(
        group.cpu_pct <= 100.0,
        "whole-machine cpu% never exceeds 100, got {}",
        group.cpu_pct,
    );
    assert!(
        !readings.contains_key(&ABSENT_PID),
        "a group with no live process is omitted, never reported as zero",
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn no_groups_means_no_readings() {
    let probe = ProcMetricsProbe::new();
    assert!(probe.sample(&[]).is_empty());
}
