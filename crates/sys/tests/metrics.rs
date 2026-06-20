//! Integration check against the real `sysinfo` adapter: it reads a live process group and
//! omits a dead one. The mock-clock sampler behaviour is covered in the core; this proves
//! the OS read itself returns sane values.

use soloist_core::MetricsProbe;
use soloist_sys::SysinfoMetricsProbe;

/// A pid that will not exist on a normal Linux system (well above `pid_max`).
const ABSENT_PID: i32 = 999_999_999;

#[test]
fn samples_the_live_test_process_and_omits_an_absent_group() {
    let probe = SysinfoMetricsProbe::new();
    let me = std::process::id() as i32;

    let readings = probe.sample(&[me, ABSENT_PID]);

    let mine = readings
        .get(&me)
        .expect("the running test process is a live group and is sampled");
    assert!(mine.rss > 0, "a live process uses some resident memory");
    assert!(
        mine.cpu_pct >= 0.0 && mine.cpu_pct.is_finite(),
        "cpu% is a sane non-negative number, got {}",
        mine.cpu_pct,
    );
    assert!(
        !readings.contains_key(&ABSENT_PID),
        "a group with no live process is omitted, never reported as zero",
    );
}

#[test]
fn no_groups_means_no_readings() {
    let probe = SysinfoMetricsProbe::new();
    assert!(probe.sample(&[]).is_empty());
}
