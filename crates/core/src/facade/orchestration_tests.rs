//! Behavioural tests for the orchestration read-model and the coordination event emissions, kept
//! out of the implementation file. They build a façade over in-memory coordination fakes and a mock
//! clock (no real time), so the snapshot assembly and the one-event-per-mutation contract are both
//! deterministic and headless.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::{IdleMode, TodoDoc, TodoStatus};
use crate::events::DomainEvent;
use crate::ids::{ProcessId, ProjectId, SessionId};
use crate::process::ProcessKind;
use crate::testing::{
    agent_registration, authentic_session, facade_with_agent_tool, terminal_registration,
    FakeKvRepo, FakeLockRepo, FakeProjectRepo, FakeScratchpadRepo, FakeSpawner, FakeTimerRepo,
    FakeTodoRepo, FakeTrustRepo, MockClock, TEST_PEER_PGID,
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
        .scoped(session)
        .bind_session_process(id)
        .expect("an authentic bind to the process the caller runs in");
    (session, id)
}

/// Registers (without starting) an agent in `project` — a node for the lineage tree.
fn agent(facade: &Facade, project: ProjectId, name: &str) -> ProcessId {
    facade
        .supervisor()
        .register(agent_registration(project, name))
}

/// A representative todo document.
fn todo_doc(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: format!("do {title}"),
        status: TodoStatus::Open,
    }
}

/// A representative scratchpad Markdown body.
fn scratchpad_body() -> String {
    "## Objective\nship it\n\n## Status\nactive".to_owned()
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
        .scoped(session)
        .todo_create(todo_doc("build"), None)
        .expect("create build")
        .view;
    let ship = facade
        .scoped(session)
        .todo_create(todo_doc("ship"), None)
        .expect("create ship")
        .view;
    let ship = facade
        .scoped(session)
        .todo_set_blockers(ship.id, vec![build.id])
        .expect("gate ship on build");
    assert!(ship.blocked, "ship is gated by build");

    // An armed fire-when-idle timer waiting on the two workers.
    let timer = facade
        .scoped(session)
        .timer_fire_when_idle(
            "integrate".into(),
            vec![worker_a, worker_b],
            IdleMode::All,
            Some(Duration::from_secs(60)),
        )
        .expect("arm the timer");

    // A held lease, a scratchpad, and a kv entry.
    facade
        .scoped(session)
        .lock_acquire("deploy", Some(Duration::from_secs(30)))
        .expect("acquire the lease");
    facade
        .scoped(session)
        .scratchpad_write("plan", scratchpad_body(), None)
        .expect("write the scratchpad");
    facade
        .scoped(session)
        .kv_set("config".into(), serde_json::json!({ "ready": true }))
        .expect("set the kv entry");

    let snap = facade
        .orchestration_snapshot(PROJECT)
        .expect("assemble the snapshot");

    // The tree carries the bound terminal owner plus the three agents. Each was registered
    // directly (no spawning lead), so each is a root mirroring the registry's status and label,
    // with no activity yet (an unsampled agent).
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
    assert_eq!(
        lead_node.label, "lead",
        "the node carries its display label"
    );
    assert_eq!(
        lead_node.parent, None,
        "a directly-registered agent is a root"
    );
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
        .scoped(session)
        .todo_create(todo_doc("only in project one"), None)
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
        .scoped(session)
        .todo_create(todo_doc("build"), None)
        .expect("create")
        .view;

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
        .scoped(session)
        .todo_create(todo_doc("build"), None)
        .expect("create")
        .view;
    let mut rx = facade.subscribe();

    facade
        .scoped(session)
        .todo_complete(todo.id)
        .expect("complete");

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
        .scoped(session)
        .lock_acquire("deploy", Some(Duration::from_secs(30)))
        .expect("acquire");
    let acquired = drain(&mut rx);
    assert_eq!(acquired.len(), 1, "one event on acquire: {acquired:?}");
    assert!(matches!(
        &acquired[0],
        DomainEvent::LeaseChanged { project, key } if *project == PROJECT && key == "deploy"
    ));

    let mut rx = facade.subscribe();
    assert!(facade
        .scoped(session)
        .lock_release("deploy")
        .expect("release"));
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
        .scoped(session)
        .timer_set("ping".into(), Some(Duration::from_secs(30)))
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
        .scoped(session)
        .timer_set("ping".into(), Some(Duration::from_secs(30)))
        .expect("set");
    let mut rx = facade.subscribe();

    assert!(facade
        .scoped(session)
        .timer_cancel(view.id)
        .expect("cancel"));

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
        .scoped(session)
        .scratchpad_write("plan", scratchpad_body(), None)
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
        .scoped(session)
        .kv_set("config".into(), serde_json::json!({ "ready": true }))
        .expect("set");

    let events = drain(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event: {events:?}");
    assert!(matches!(
        &events[0],
        DomainEvent::KvChanged { project, key } if *project == PROJECT && key == "config"
    ));
}

#[tokio::test]
async fn a_worker_spawned_by_a_bound_lead_nests_under_it() {
    let (facade, project) = facade_with_agent_tool();
    // A lead agent in the project, with a session bound to it as its own MCP client would bind.
    let lead = agent(&facade, project, "lead");
    let session = authentic_session(&facade, lead, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(lead)
        .expect("bind the lead to its own process");

    let worker = facade
        .scoped(session)
        .spawn_agent("worker", Vec::new())
        .expect("spawn the worker under the lead");

    let snap = facade
        .orchestration_snapshot(project)
        .expect("assemble the snapshot");
    let worker_node = snap
        .agents
        .iter()
        .find(|node| node.id == worker)
        .expect("the worker is a node");
    assert_eq!(
        worker_node.parent,
        Some(lead),
        "a worker spawned by a bound lead nests under it",
    );
}

#[tokio::test]
async fn an_unbound_spawn_is_a_root() {
    let (facade, project) = facade_with_agent_tool();
    // A session with no bound process: it still resolves its scope to the sole project, but has
    // no lead to spawn under.
    let session = facade.open_session(None);

    let worker = facade
        .scoped(session)
        .spawn_agent("worker", Vec::new())
        .expect("spawn the worker with no bound lead");

    let snap = facade
        .orchestration_snapshot(project)
        .expect("assemble the snapshot");
    let worker_node = snap
        .agents
        .iter()
        .find(|node| node.id == worker)
        .expect("the worker is a node");
    assert_eq!(
        worker_node.parent, None,
        "an unbound spawn has no parent and is a root",
    );
}

#[tokio::test]
async fn closing_a_lead_re_parents_its_worker_to_root() {
    let (facade, project) = facade_with_agent_tool();
    let lead = agent(&facade, project, "lead");
    let session = authentic_session(&facade, lead, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(lead)
        .expect("bind the lead to its own process");
    let worker = facade
        .scoped(session)
        .spawn_agent("worker", Vec::new())
        .expect("spawn the worker under the lead");

    // The lead leaves the registry; its worker must not be stranded.
    facade
        .supervisor()
        .close(lead)
        .await
        .expect("close the lead");

    let snap = facade
        .orchestration_snapshot(project)
        .expect("assemble the snapshot");
    assert!(
        snap.agents.iter().all(|node| node.id != lead),
        "the closed lead leaves the tree",
    );
    let worker_node = snap
        .agents
        .iter()
        .find(|node| node.id == worker)
        .expect("the worker is still a node");
    assert_eq!(
        worker_node.parent, None,
        "the worker re-parents to root when its lead closes",
    );
}

#[tokio::test]
async fn lineage_edges_omits_an_edge_whose_parent_left_the_registry() {
    let (facade, project) = facade_with_agent_tool();
    let lead = agent(&facade, project, "lead");
    let session = authentic_session(&facade, lead, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(lead)
        .expect("bind the lead to its own process");
    let worker = facade
        .scoped(session)
        .spawn_agent("worker", Vec::new())
        .expect("spawn the worker under the lead");

    assert_eq!(
        facade.lineage_edges(),
        vec![LineageEdge {
            child: worker,
            parent: lead,
        }],
        "a live worker-lead pair is one edge",
    );

    facade
        .supervisor()
        .close(lead)
        .await
        .expect("close the lead");

    // The recorded parent survives the close (the strict spawn gate reads it), but the edge
    // read re-roots: a pair is an edge only while both ends are in the registry.
    assert_eq!(facade.lineage.parent_of(worker), Some(lead));
    assert_eq!(
        facade.lineage_edges(),
        Vec::new(),
        "a closed lead's edge leaves the read",
    );
}
