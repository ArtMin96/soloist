//! [`AttentionRegistry`] behaviour: what it remembers, what it forgets, and when it reports a
//! real change. The snapshot is the single source every surface renders unread from, so these
//! assert the snapshot rather than the registry's internals.

use super::*;

const WEB: ProcessId = ProcessId::from_raw(1);
const API: ProcessId = ProcessId::from_raw(2);

/// Far enough past the ceiling to show the count stops rather than merely slows.
const OVERSHOOT: u32 = 500;

/// A registry one process has alerted on more times than it will be counted for.
fn storm() -> AttentionRegistry {
    let registry = AttentionRegistry::new();
    for _ in 0..MAX_UNREAD_PER_PROCESS + OVERSHOOT {
        registry.raise(WEB, AttentionKind::Crashed);
    }
    registry
}

/// A snapshot as the surfaces receive it — what the registry costs to send, not what it holds.
fn serialised(snapshot: &AttentionSnapshot) -> String {
    serde_json::to_string(snapshot).expect("a snapshot serialises")
}

/// What the snapshot reports for `process`, or nothing when it has nothing waiting.
fn entry_for(snapshot: &AttentionSnapshot, process: ProcessId) -> Option<ProcessAttention> {
    snapshot
        .processes
        .iter()
        .find(|entry| entry.process == process)
        .copied()
}

#[test]
fn a_new_registry_has_nothing_unread() {
    let snapshot = AttentionRegistry::new().snapshot();

    assert_eq!(snapshot.total, 0);
    assert!(snapshot.processes.is_empty());
}

#[test]
fn raising_records_the_kind_against_its_process() {
    let registry = AttentionRegistry::new();

    registry.raise(WEB, AttentionKind::Crashed);

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.total, 1);
    assert_eq!(
        entry_for(&snapshot, WEB),
        Some(ProcessAttention {
            process: WEB,
            kind: AttentionKind::Crashed,
            alerts: 1,
        })
    );
}

#[test]
fn the_kind_reported_is_the_one_that_started_the_run() {
    let registry = AttentionRegistry::new();

    registry.raise(WEB, AttentionKind::TerminalBell);
    registry.raise(WEB, AttentionKind::Crashed);

    // The marker is how a user finds the row again, so it names what first asked for them; a later
    // alert adds to the count without renaming what is waiting.
    let entry = entry_for(&registry.snapshot(), WEB).expect("web has something waiting");
    assert_eq!(entry.kind, AttentionKind::TerminalBell);
    assert_eq!(entry.alerts, 2);
}

#[test]
fn the_total_counts_alerts_not_processes() {
    let registry = AttentionRegistry::new();

    registry.raise(WEB, AttentionKind::Crashed);
    registry.raise(WEB, AttentionKind::TerminalBell);
    registry.raise(API, AttentionKind::AgentPermission);

    // Two processes, three alerts: the badge shows how much is waiting, not how many rows carry
    // something.
    assert_eq!(registry.snapshot().total, 3);
}

#[test]
fn the_same_kind_twice_counts_twice() {
    let registry = AttentionRegistry::new();

    // Two crashes are two things that happened. Collapsing them would under-report a process that
    // is failing repeatedly — the case the count matters most for.
    registry.raise(WEB, AttentionKind::Crashed);
    registry.raise(WEB, AttentionKind::Crashed);

    assert_eq!(registry.snapshot().total, 2);
}

#[test]
fn the_count_is_truthful_past_the_display_cap() {
    let registry = AttentionRegistry::new();

    for _ in 0..150 {
        registry.raise(WEB, AttentionKind::Crashed);
    }

    // Capping at "99+" is a rendering choice; the core must not truncate what it counts, or every
    // surface inherits one renderer's decision.
    assert_eq!(registry.snapshot().total, 150);
}

#[test]
fn the_count_stops_at_the_ceiling_however_much_a_process_raises() {
    // The feed is a child process's own output — one alert per BEL byte it prints — so what it can
    // drive this count to is whatever it can print. It stops where a larger number stops telling
    // the user anything.
    assert_eq!(storm().snapshot().total, MAX_UNREAD_PER_PROCESS as usize);
}

#[test]
fn a_process_that_never_stops_alerting_snapshots_the_same_size_as_one_that_alerted_once() {
    let quiet = AttentionRegistry::new();
    quiet.raise(WEB, AttentionKind::Crashed);

    let once = serialised(&quiet.snapshot());
    let storm = serialised(&storm().snapshot());

    // Every alert wakes every surface to re-read this snapshot, so one that carried the history
    // would cost more to build and send the longer the storm ran. All a storm may add is the
    // digits of the count itself, in each of the two places it is written: the process's own and
    // the total.
    let digits = MAX_UNREAD_PER_PROCESS.to_string().len();
    assert!(
        storm.len() <= once.len() + 2 * digits,
        "a storm snapshotted to {} bytes against {} for a single alert",
        storm.len(),
        once.len()
    );
}

#[test]
fn clearing_one_process_leaves_the_others() {
    let registry = AttentionRegistry::new();
    registry.raise(WEB, AttentionKind::Crashed);
    registry.raise(API, AttentionKind::AgentPermission);

    registry.clear(WEB);

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.total, 1);
    assert_eq!(entry_for(&snapshot, WEB), None);
    assert_eq!(
        entry_for(&snapshot, API),
        Some(ProcessAttention {
            process: API,
            kind: AttentionKind::AgentPermission,
            alerts: 1,
        })
    );
}

#[test]
fn clearing_everything_empties_the_snapshot() {
    let registry = AttentionRegistry::new();
    registry.raise(WEB, AttentionKind::Crashed);
    registry.raise(API, AttentionKind::AgentPermission);

    registry.clear_all();

    assert_eq!(registry.snapshot().total, 0);
}

#[test]
fn clearing_what_is_already_clear_reports_no_change() {
    let registry = AttentionRegistry::new();

    // The callers announce a change on the bus, so a clear that changed nothing must not make them
    // wake every surface for a re-query that returns the same answer.
    assert!(!registry.clear(WEB));
    assert!(!registry.clear_all());
}

#[test]
fn a_clear_that_removes_something_reports_a_change() {
    let registry = AttentionRegistry::new();
    registry.raise(WEB, AttentionKind::Crashed);

    assert!(registry.clear(WEB));

    registry.raise(API, AttentionKind::Crashed);
    assert!(registry.clear_all());
}
