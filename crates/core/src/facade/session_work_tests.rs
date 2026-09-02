//! What [`Facade::session_work`] reports, and what the recording helpers attribute an access to —
//! built over in-memory fakes so the join against live todos/scratchpads and the per-run recording
//! are both exercised end to end.

use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::{TodoDoc, TodoStatus};
use crate::events::DomainEvent;
use crate::ids::ProjectId;
use crate::ports::ProjectRepo;
use crate::process::ProcStatus;
use crate::testing::{
    agent_registration, authentic_session, bound_agent, drain, next_matching,
    terminal_registration, FakeProjectRepo, FakeScratchpadRepo, FakeSpawner, FakeTodoRepo,
    FakeTrustRepo, MockClock, TEST_PEER_PGID,
};
use crate::PeerCredentials;

/// A façade with one loaded project and its todo/scratchpad stores wired to in-memory fakes, so
/// the session-work join has real documents to read. Returns the façade and the project's id.
fn facade() -> (Facade, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects
        .upsert(Path::new("/"), Some("proj"), None)
        .expect("seed a project")
        .id;
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(MockClock::new()),
            Arc::new(FakeTrustRepo::new()),
            projects,
        )
        .todo_repo(Arc::new(FakeTodoRepo::new()))
        .scratchpad_repo(Arc::new(FakeScratchpadRepo::new()))
        .build(),
    );
    (facade, project)
}

fn todo_doc(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: format!("do {title}"),
        status: TodoStatus::Open,
    }
}

#[test]
fn names_the_todo_a_bound_session_read_and_the_one_it_wrote() {
    let (facade, project) = facade();
    // Seeded via the local-UI path, which records nothing, so each starts with a clean slate.
    let read_target = facade
        .todo_create_in(project, todo_doc("read me"), None)
        .expect("seed a todo to read")
        .id;
    let write_target = facade
        .todo_create_in(project, todo_doc("write me"), None)
        .expect("seed a todo to write")
        .id;
    let (process, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);

    facade
        .scoped(session)
        .todo_get(read_target)
        .expect("read it");
    facade
        .scoped(session)
        .todo_add_tag(write_target, "urgent")
        .expect("write it");

    let work = facade
        .session_work(process)
        .expect("query")
        .expect("work was recorded");
    let read_entry = work
        .todos
        .iter()
        .find(|todo| todo.id == read_target)
        .expect("the read todo is present");
    assert_eq!(read_entry.access, Some(AccessKind::Loaded));
    let write_entry = work
        .todos
        .iter()
        .find(|todo| todo.id == write_target)
        .expect("the written todo is present");
    assert_eq!(write_entry.access, Some(AccessKind::Worked));
}

#[test]
fn reports_a_lock_the_process_holds_as_current_work() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("ship"), None)
        .expect("seed")
        .id;
    let (process, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);

    facade.scoped(session).todo_lock(todo).expect("lock it");

    let work = facade
        .session_work(process)
        .expect("query")
        .expect("work was recorded");
    let entry = work
        .todos
        .iter()
        .find(|session_todo| session_todo.id == todo)
        .expect("the locked todo is present");
    assert!(entry.locked, "a held lock reads as current work");
}

#[test]
fn drops_a_recorded_document_that_no_longer_exists() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("temp"), None)
        .expect("seed")
        .id;
    let (process, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    facade.scoped(session).todo_get(todo).expect("read it");

    assert!(facade.scoped(session).todo_delete(todo).expect("delete it"));

    let work = facade
        .session_work(process)
        .expect("query")
        .expect("the process is still tracked, though it has nothing live");
    assert!(
        work.todos.is_empty(),
        "a deleted todo drops out of the join rather than surviving as a stale title"
    );
}

#[test]
fn an_unbound_caller_records_nothing() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("open"), None)
        .expect("seed")
        .id;
    // A process exists in the project so there is something a bug could wrongly attribute the
    // read to, but this session is never bound to it.
    let agent = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let session = facade.open_session(PeerCredentials::unauthenticated());

    facade
        .scoped(session)
        .todo_get(todo)
        .expect("an unbound caller may still read within its resolved scope");

    assert_eq!(
        facade.session_work(agent).expect("query"),
        None,
        "an unbound caller's read is attributed to no process"
    );
}

#[test]
fn an_external_caller_records_nothing() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("open"), None)
        .expect("seed")
        .id;
    let agent = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let session = facade.open_session(PeerCredentials::unauthenticated());
    facade
        .scoped(session)
        .register_agent("an external tool".into());

    facade
        .scoped(session)
        .todo_get(todo)
        .expect("an external caller may still read within its resolved scope");

    assert_eq!(
        facade.session_work(agent).expect("query"),
        None,
        "an external caller's read is attributed to no process"
    );
}

#[tokio::test]
async fn stopping_the_process_clears_its_record() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("ship"), None)
        .expect("seed")
        .id;
    let id = facade
        .supervisor()
        .register(terminal_registration(project, "term", "sleep 60"));
    facade
        .supervisor()
        .start(id)
        .expect("ungated terminal starts");
    let mut rx = facade.subscribe();
    next_matching(&mut rx, |event| {
        matches!(event, DomainEvent::ProcessStatusChanged { to, .. } if *to == ProcStatus::Running)
    })
    .await;

    let session = authentic_session(&facade, id, TEST_PEER_PGID);
    facade
        .scoped(session)
        .bind_session_process(id)
        .expect("bind");
    facade.scoped(session).todo_get(todo).expect("read it");
    assert!(
        facade.session_work(id).expect("query").is_some(),
        "the access is recorded while the process runs"
    );

    assert!(facade.supervisor().stop(id));
    next_matching(&mut rx, |event| {
        matches!(event, DomainEvent::ProcessStatusChanged { to, .. } if *to == ProcStatus::Stopped)
    })
    .await;

    assert_eq!(
        facade.session_work(id).expect("query"),
        None,
        "the record ends when the process does"
    );
}

#[test]
fn one_todo_get_emits_exactly_one_session_work_changed_and_a_repeat_emits_none() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("ship"), None)
        .expect("seed")
        .id;
    let (process, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let mut rx = facade.subscribe();

    facade.scoped(session).todo_get(todo).expect("read");
    let events = drain(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event: {events:?}");
    assert!(matches!(
        &events[0],
        DomainEvent::SessionWorkChanged { process: got } if *got == process
    ));

    facade.scoped(session).todo_get(todo).expect("read again");
    assert!(
        drain(&mut rx).is_empty(),
        "an identical repeat emits nothing"
    );
}
