//! [`SessionActivity`] behaviour: what a record holds, how it grows and caps, and when it actually
//! changes — the source the session-work read model and the process-close hook both depend on.

use super::*;
use crate::testing::drain;

const LEAD: ProcessId = ProcessId::from_raw(1);
const OTHER: ProcessId = ProcessId::from_raw(2);

fn registry() -> SessionActivity {
    SessionActivity::new(EventBus::new(16))
}

#[test]
fn a_read_records_loaded_and_a_write_records_worked() {
    let registry = registry();

    registry.record_todo(LEAD, TodoId::from_raw(1), AccessKind::Loaded);
    registry.record_todo(LEAD, TodoId::from_raw(2), AccessKind::Worked);

    assert_eq!(
        registry.todos(LEAD),
        vec![
            (TodoId::from_raw(1), AccessKind::Loaded),
            (TodoId::from_raw(2), AccessKind::Worked),
        ]
    );
}

#[test]
fn a_write_after_a_read_upgrades_to_worked_and_a_later_read_never_downgrades_it() {
    let registry = registry();
    let todo = TodoId::from_raw(1);

    registry.record_todo(LEAD, todo, AccessKind::Loaded);
    registry.record_todo(LEAD, todo, AccessKind::Worked);
    assert_eq!(registry.todos(LEAD), vec![(todo, AccessKind::Worked)]);

    // A write already told the fuller story of what happened this run; a later read must not
    // erase it.
    registry.record_todo(LEAD, todo, AccessKind::Loaded);
    assert_eq!(registry.todos(LEAD), vec![(todo, AccessKind::Worked)]);
}

#[test]
fn re_touching_an_entry_updates_it_in_place_without_reordering() {
    let registry = registry();
    let first = TodoId::from_raw(1);
    let second = TodoId::from_raw(2);

    registry.record_todo(LEAD, first, AccessKind::Loaded);
    registry.record_todo(LEAD, second, AccessKind::Loaded);
    registry.record_todo(LEAD, first, AccessKind::Worked);

    assert_eq!(
        registry.todos(LEAD),
        vec![(first, AccessKind::Worked), (second, AccessKind::Loaded),],
        "re-touching the first entry must not move it to the end"
    );
}

#[test]
fn past_the_cap_the_oldest_entry_is_evicted_and_the_newest_kept() {
    let registry = registry();

    for raw in 0..(MAX_SESSION_DOCUMENTS_PER_PROCESS as u64 + 10) {
        registry.record_todo(LEAD, TodoId::from_raw(raw), AccessKind::Loaded);
    }

    let recorded = registry.todos(LEAD);
    assert_eq!(recorded.len(), MAX_SESSION_DOCUMENTS_PER_PROCESS);
    let ids: Vec<u64> = recorded.iter().map(|(id, _)| id.get()).collect();
    assert_eq!(ids.first(), Some(&10), "the oldest ten were evicted");
    assert_eq!(
        ids.last(),
        Some(&(MAX_SESSION_DOCUMENTS_PER_PROCESS as u64 + 9)),
        "the newest entry is kept"
    );
}

#[test]
fn todos_and_scratchpads_are_capped_and_recorded_independently() {
    let registry = registry();

    registry.record_todo(LEAD, TodoId::from_raw(1), AccessKind::Loaded);
    registry.record_scratchpad(LEAD, ScratchpadId::from_raw(9), AccessKind::Worked);

    assert_eq!(
        registry.todos(LEAD),
        vec![(TodoId::from_raw(1), AccessKind::Loaded)]
    );
    assert_eq!(
        registry.scratchpads(LEAD),
        vec![(ScratchpadId::from_raw(9), AccessKind::Worked)]
    );
}

#[test]
fn forgetting_one_process_leaves_another_untouched() {
    let registry = registry();
    registry.record_todo(LEAD, TodoId::from_raw(1), AccessKind::Loaded);
    registry.record_todo(OTHER, TodoId::from_raw(2), AccessKind::Loaded);

    registry.forget(LEAD);

    assert!(registry.todos(LEAD).is_empty());
    assert_eq!(
        registry.todos(OTHER),
        vec![(TodoId::from_raw(2), AccessKind::Loaded)]
    );
}

#[test]
fn releasing_all_forgets_the_process() {
    let registry = registry();
    registry.record_todo(LEAD, TodoId::from_raw(1), AccessKind::Loaded);

    LockReleaser::release_all(&registry, LEAD);

    assert!(registry.todos(LEAD).is_empty());
}

#[test]
fn a_repeat_of_the_same_access_publishes_no_event() {
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();
    let registry = SessionActivity::new(bus);
    let todo = TodoId::from_raw(1);

    registry.record_todo(LEAD, todo, AccessKind::Loaded);
    assert_eq!(drain(&mut rx).len(), 1, "the first access is a real change");

    registry.record_todo(LEAD, todo, AccessKind::Loaded);
    assert!(
        drain(&mut rx).is_empty(),
        "a repeat of the same access changes nothing"
    );
}
