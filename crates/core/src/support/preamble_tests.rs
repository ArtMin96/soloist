use super::*;
use crate::ids::{ProcessId, ProjectId};

fn project(name: Option<&str>) -> ProjectRef {
    ProjectRef {
        id: ProjectId::from_raw(7),
        name: name.map(str::to_string),
    }
}

#[test]
fn a_worker_is_told_who_spawned_it_and_where_it_is() {
    let preamble =
        orchestration_preamble(&project(Some("storefront")), Some(ProcessId::from_raw(4)));
    assert!(preamble.starts_with(MARKER), "{preamble}");
    assert!(
        preamble.contains("process #4"),
        "the worker learns which process to report back to: {preamble}"
    );
    assert!(
        preamble.contains("\"storefront\" (#7)"),
        "the worker learns which project its tools act on: {preamble}"
    );
    assert!(
        preamble.contains("report_to_lead"),
        "the worker learns how to hand its result back: {preamble}"
    );
}

#[test]
fn a_worker_is_told_that_finishing_quietly_signals_nothing() {
    // The load-bearing sentence. Completion is explicit: a worker that stops talking looks
    // exactly like one that is thinking, so a lead reading silence as done acts on work nobody
    // did. The preamble has to say both halves — call the tool, and quiet is not a substitute.
    let preamble =
        orchestration_preamble(&project(Some("storefront")), Some(ProcessId::from_raw(4)));
    assert!(
        preamble.contains("must call `report_to_lead`"),
        "the worker is told to report, not asked: {preamble}"
    );
    assert!(
        preamble.contains("Going quiet does not"),
        "the worker is told that silence signals nothing: {preamble}"
    );
}

#[test]
fn a_worker_can_use_the_coordination_primitives_from_the_preamble_alone() {
    // The whole point of the opening turn: an agent with no skill and no project file loaded
    // still knows the primitives exist and what each is for.
    let preamble =
        orchestration_preamble(&project(Some("storefront")), Some(ProcessId::from_raw(4)));
    for topic in ["whoami", "todos", "scratchpads", "timers", "locks"] {
        assert!(
            preamble.contains(topic),
            "the preamble carries the {topic} capability: {preamble}"
        );
    }
}

#[test]
fn a_root_spawn_names_no_lead() {
    // A caller Soloist could not name spawned it, so there is no lead to point the worker at —
    // and inventing one would send its report to a process that never asked for it.
    let preamble = orchestration_preamble(&project(Some("storefront")), None);
    assert!(!preamble.contains("spawned by"), "{preamble}");
    assert!(preamble.contains("\"storefront\" (#7)"), "{preamble}");
}

#[test]
fn a_project_with_no_readable_name_is_still_named_by_id() {
    // The name is a best-effort durable read that can come back empty; the scope it identifies
    // must not vanish with it.
    let preamble = orchestration_preamble(&project(None), Some(ProcessId::from_raw(4)));
    assert!(preamble.contains("project #7"), "{preamble}");
}
