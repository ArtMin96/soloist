//! [`AttentionRegistry`] behaviour: what it remembers, what it forgets, and when it reports a
//! real change. The snapshot is the single source every surface renders unread from, so these
//! assert the snapshot rather than the registry's internals.

use super::*;

const WEB: ProcessId = ProcessId::from_raw(1);
const API: ProcessId = ProcessId::from_raw(2);

/// The kinds recorded against `process`, in the order the snapshot reports them.
fn kinds_for(snapshot: &AttentionSnapshot, process: ProcessId) -> Vec<AttentionKind> {
    snapshot
        .processes
        .iter()
        .find(|entry| entry.process == process)
        .map(|entry| entry.kinds.clone())
        .unwrap_or_default()
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
    assert_eq!(kinds_for(&snapshot, WEB), vec![AttentionKind::Crashed]);
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
fn clearing_one_process_leaves_the_others() {
    let registry = AttentionRegistry::new();
    registry.raise(WEB, AttentionKind::Crashed);
    registry.raise(API, AttentionKind::AgentPermission);

    registry.clear(WEB);

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.total, 1);
    assert!(kinds_for(&snapshot, WEB).is_empty());
    assert_eq!(
        kinds_for(&snapshot, API),
        vec![AttentionKind::AgentPermission]
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
