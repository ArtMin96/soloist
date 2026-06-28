//! Behavioural tests for the orchestration read-model and the coordination event emissions, kept
//! out of the implementation file. They build a façade over in-memory coordination fakes and a mock
//! clock (no real time), so the snapshot assembly and the one-event-per-mutation contract are both
//! deterministic and headless.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use super::*;
use crate::coordination::{IdleMode, ScratchpadDoc, TodoDoc, TodoStatus};
use crate::events::DomainEvent;
use crate::ids::{ProcessId, ProjectId, SessionId};
use crate::ports::{CorePorts, PtySize, SpawnSpec};
use crate::process::ProcessKind;
use crate::supervisor::Registration;
use crate::testing::{
    authentic_session, terminal_registration, FakeKvRepo, FakeLockRepo, FakeProjectRepo,
    FakeScratchpadRepo, FakeSpawner, FakeTimerRepo, FakeTodoRepo, FakeTrustRepo, MockClock,
    TEST_PEER_PGID,
};

const PROJECT: ProjectId = ProjectId::from_raw(1);

/// A façade over in-memory fakes with every coordination store wired, so the read-model and the
/// event emissions can be exercised end to end. A mock clock keeps lease expiry deterministic.
fn facade() -> Facade {
    Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(MockClock::new()),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .lock_repo(Arc::new(FakeLockRepo::new()))
        .timer_repo(Arc::new(FakeTimerRepo::new()))
        .scratchpad_repo(Arc::new(FakeScratchpadRepo::new()))
        .todo_repo(Arc::new(FakeTodoRepo::new()))
        .kv_repo(Arc::new(FakeKvRepo::new()))
        .build(),
    )
}

/// Binds a session to a fresh terminal in `project`, as the UDS adapter would for an MCP client
/// running inside that process — so the session has both an effective project and a record owner.
fn bound_session(facade: &Facade, project: ProjectId) -> (SessionId, ProcessId) {
    let id = facade
        .supervisor()
        .register(terminal_registration(project, "term", "sleep 60"));
    let session = authentic_session(facade, id, TEST_PEER_PGID);
    facade
        .bind_session_process(session, id)
        .expect("an authentic bind to the process the caller runs in");
    (session, id)
}

/// Registers (without starting) an agent in `project` — a node for the lineage tree.
fn agent(facade: &Facade, project: ProjectId, name: &str) -> ProcessId {
    facade.supervisor().register(Registration::launched(
        project,
        ProcessKind::Agent,
        name,
        SpawnSpec {
            command: "agent".into(),
            working_dir: ".".into(),
            env: BTreeMap::new(),
            size: PtySize::default(),
        },
    ))
}

/// A well-formed disciplined todo document.
fn todo_doc(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        description: format!("do {title}"),
        acceptance_criteria: vec!["it works".into()],
        risks: vec!["none identified".into()],
        status: TodoStatus::Open,
    }
}

/// A well-formed disciplined scratchpad document.
fn scratchpad_doc() -> ScratchpadDoc {
    ScratchpadDoc {
        objective: "ship it".into(),
        context: "the plan".into(),
        plan: vec!["step one".into()],
        acceptance_criteria: vec!["it ships".into()],
        risks: vec!["none identified".into()],
        status: "active".into(),
        notes: None,
    }
}

/// Every event currently buffered for `rx`, drained synchronously — the events emitted by the one
/// mutation performed since subscribing.
fn drain(rx: &mut broadcast::Receiver<DomainEvent>) -> Vec<DomainEvent> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

#[test]
fn the_snapshot_projects_the_tree_todos_timers_leases_scratchpads_and_kv_for_a_project() {
    let facade = facade();
    let (session, owner) = bound_session(&facade, PROJECT);

    // A lead and two workers in the project.
    let lead = agent(&facade, PROJECT, "lead");
    let worker_a = agent(&facade, PROJECT, "worker-a");
    let worker_b = agent(&facade, PROJECT, "worker-b");

    // A blocked todo: `ship` is gated by `build`.
    let build = facade
        .todo_create(session, todo_doc("build"))
        .expect("create build");
    let ship = facade
        .todo_create(session, todo_doc("ship"))
        .expect("create ship");
    let ship = facade
        .todo_set_blockers(session, ship.id, vec![build.id])
        .expect("gate ship on build");
    assert!(ship.blocked, "ship is gated by build");

    // An armed fire-when-idle timer waiting on the two workers.
    let timer = facade
        .timer_fire_when_idle(
            session,
            "integrate".into(),
            vec![worker_a, worker_b],
            IdleMode::All,
            Some(Duration::from_secs(60)),
        )
        .expect("arm the timer");

    // A held lease, a scratchpad, and a kv entry.
    facade
        .lock_acquire(session, "deploy", Some(Duration::from_secs(30)))
        .expect("acquire the lease");
    facade
        .scratchpad_write(session, "plan", scratchpad_doc(), None)
        .expect("write the scratchpad");
    facade
        .kv_set(
            session,
            "config".into(),
            serde_json::json!({ "ready": true }),
        )
        .expect("set the kv entry");

    let snap = facade
        .orchestration_snapshot(PROJECT)
        .expect("assemble the snapshot");

    // The tree carries the bound terminal owner plus the three agents, each a root mirroring the
    // registry's status, with no activity yet (an unsampled agent) and no lineage recorded yet.
    assert_eq!(snap.project, PROJECT);
    let ids: Vec<ProcessId> = snap.agents.iter().map(|node| node.id).collect();
    for id in [owner, lead, worker_a, worker_b] {
        assert!(ids.contains(&id), "the tree includes {id:?}");
    }
    let lead_node = snap
        .agents
        .iter()
        .find(|node| node.id == lead)
        .expect("the lead is a node");
    assert_eq!(lead_node.kind, ProcessKind::Agent);
    assert_eq!(lead_node.parent, None, "spawn lineage is not recorded yet");
    assert_eq!(
        lead_node.activity, None,
        "an unsampled agent has no activity"
    );
    assert_eq!(
        lead_node.status,
        facade.process_view(lead).expect("the lead's view").status,
        "the node mirrors the registry status",
    );

    // Todos: both present, `ship` reads blocked with its blocker.
    assert_eq!(snap.todos.len(), 2);
    let ship_view = snap
        .todos
        .iter()
        .find(|todo| todo.id == ship.id)
        .expect("ship is in the snapshot");
    assert!(ship_view.blocked, "the blocked todo reads blocked");
    assert_eq!(ship_view.blockers, vec![build.id]);

    // Timers, leases, scratchpads, kv each project their one record.
    assert_eq!(snap.timers.len(), 1);
    assert_eq!(snap.timers[0].id, timer.timer.id);
    assert_eq!(snap.leases.len(), 1);
    assert_eq!(snap.leases[0].key, "deploy");
    assert_eq!(snap.leases[0].owner, owner);
    assert_eq!(snap.scratchpads.len(), 1);
    assert_eq!(snap.scratchpads[0].name, "plan");
    assert_eq!(snap.kv.len(), 1);
    assert_eq!(snap.kv[0].key, "config");
}

#[test]
fn the_snapshot_is_scoped_to_its_project() {
    let facade = facade();
    let (session, _owner) = bound_session(&facade, PROJECT);
    facade
        .todo_create(session, todo_doc("only in project one"))
        .expect("create a todo in project one");

    // A different project shares no processes or coordination state.
    let other = facade
        .orchestration_snapshot(ProjectId::from_raw(2))
        .expect("snapshot the other project");
    assert!(other.agents.is_empty(), "no processes in the other project");
    assert!(other.todos.is_empty(), "no todos leak across projects");
}

#[test]
fn creating_a_todo_emits_one_todo_changed() {
    let facade = facade();
    let (session, _owner) = bound_session(&facade, PROJECT);
    let mut rx = facade.subscribe();

    let todo = facade
        .todo_create(session, todo_doc("build"))
        .expect("create");

    let events = drain(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event: {events:?}");
    assert!(matches!(
        &events[0],
        DomainEvent::TodoChanged { project, id } if *project == PROJECT && *id == todo.id
    ));
}

#[test]
fn completing_a_todo_emits_one_todo_changed() {
    let facade = facade();
    let (session, _owner) = bound_session(&facade, PROJECT);
    let todo = facade
        .todo_create(session, todo_doc("build"))
        .expect("create");
    let mut rx = facade.subscribe();

    facade.todo_complete(session, todo.id).expect("complete");

    let events = drain(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event: {events:?}");
    assert!(matches!(
        &events[0],
        DomainEvent::TodoChanged { project, id } if *project == PROJECT && *id == todo.id
    ));
}

#[test]
fn acquiring_then_releasing_a_lease_each_emit_one_lease_changed() {
    let facade = facade();
    let (session, _owner) = bound_session(&facade, PROJECT);

    let mut rx = facade.subscribe();
    facade
        .lock_acquire(session, "deploy", Some(Duration::from_secs(30)))
        .expect("acquire");
    let acquired = drain(&mut rx);
    assert_eq!(acquired.len(), 1, "one event on acquire: {acquired:?}");
    assert!(matches!(
        &acquired[0],
        DomainEvent::LeaseChanged { project, key } if *project == PROJECT && key == "deploy"
    ));

    let mut rx = facade.subscribe();
    assert!(facade.lock_release(session, "deploy").expect("release"));
    let released = drain(&mut rx);
    assert_eq!(released.len(), 1, "one event on release: {released:?}");
    assert!(matches!(
        &released[0],
        DomainEvent::LeaseChanged { project, key } if *project == PROJECT && key == "deploy"
    ));
}

#[test]
fn arming_a_timer_emits_one_timer_armed() {
    let facade = facade();
    let (session, owner) = bound_session(&facade, PROJECT);
    let mut rx = facade.subscribe();

    let view = facade
        .timer_set(session, "ping".into(), Some(Duration::from_secs(30)))
        .expect("set");

    let events = drain(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event: {events:?}");
    assert!(matches!(
        &events[0],
        DomainEvent::TimerArmed { owner: got, id } if *got == owner && *id == view.id
    ));
}

#[test]
fn cancelling_a_timer_emits_one_timer_cleared() {
    let facade = facade();
    let (session, owner) = bound_session(&facade, PROJECT);
    let view = facade
        .timer_set(session, "ping".into(), Some(Duration::from_secs(30)))
        .expect("set");
    let mut rx = facade.subscribe();

    assert!(facade.timer_cancel(session, view.id).expect("cancel"));

    let events = drain(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event: {events:?}");
    assert!(matches!(
        &events[0],
        DomainEvent::TimerCleared { owner: got, id } if *got == owner && *id == view.id
    ));
}

#[test]
fn writing_a_scratchpad_emits_one_scratchpad_changed() {
    let facade = facade();
    let (session, _owner) = bound_session(&facade, PROJECT);
    let mut rx = facade.subscribe();

    facade
        .scratchpad_write(session, "plan", scratchpad_doc(), None)
        .expect("write");

    let events = drain(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event: {events:?}");
    assert!(matches!(
        &events[0],
        DomainEvent::ScratchpadChanged { project, name } if *project == PROJECT && name == "plan"
    ));
}

#[test]
fn setting_a_kv_entry_emits_one_kv_changed() {
    let facade = facade();
    let (session, _owner) = bound_session(&facade, PROJECT);
    let mut rx = facade.subscribe();

    facade
        .kv_set(
            session,
            "config".into(),
            serde_json::json!({ "ready": true }),
        )
        .expect("set");

    let events = drain(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event: {events:?}");
    assert!(matches!(
        &events[0],
        DomainEvent::KvChanged { project, key } if *project == PROJECT && key == "config"
    ));
}
