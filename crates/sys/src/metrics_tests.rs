//! Unit tests for the CPU normalisation, kept out of the implementation file. The OS read
//! itself is exercised against a real process group in `tests/metrics.rs`.

use super::cpu_percent;

const CLK_TCK: f64 = 100.0;

#[test]
fn full_use_of_every_core_reads_one_hundred_percent() {
    // 8 cores fully busy for 1s = 8 core-seconds = 8 * 100 ticks; normalised to the machine
    // that is 100%, never 800% (the per-core convention).
    let pct = cpu_percent(8 * 100, CLK_TCK, 1.0, 8.0);
    assert!((pct - 100.0).abs() < 0.01, "expected ~100%, got {pct}");
}

#[test]
fn two_busy_cores_on_an_eight_core_machine_read_a_quarter() {
    // 2 core-seconds over 1s of wall time on 8 cores = 25%.
    let pct = cpu_percent(2 * 100, CLK_TCK, 1.0, 8.0);
    assert!((pct - 25.0).abs() < 0.01, "expected ~25%, got {pct}");
}

#[test]
fn an_idle_group_reads_zero() {
    assert_eq!(cpu_percent(0, CLK_TCK, 1.0, 8.0), 0.0);
}

#[test]
fn the_first_interval_reads_zero() {
    // No elapsed time yet (the priming sample) yields 0 rather than a divide-by-zero.
    assert_eq!(cpu_percent(500, CLK_TCK, 0.0, 8.0), 0.0);
}
