//! What [`Facade::project_work`] reports, and what the recording helpers attribute an access to —
//! built over in-memory fakes so the join against live todos/scratchpads, the process registry, and
//! the per-run recording are all exercised end to end.

use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::composition::CorePorts;
use crate::coordination::{TodoDoc, TodoStatus};
use crate::events::DomainEvent;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::ProjectRepo;
use crate::testing::{
    agent_registration, bound_agent, drain, FakeProjectRepo, FakeScratchpadRepo, FakeSpawner,
    FakeTodoRepo, FakeTrustRepo, MockClock, TEST_PEER_PGID,
};
use crate::PeerCredentials;

/// A façade with one loaded project and its todo/scratchpad stores wired to in-memory fakes, so
/// the project-work join has real documents to read. Returns the façade and the project's id.
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

/// A façade with two loaded projects, for the cross-project isolation test.
fn facade_with_two_projects() -> (Facade, ProjectId, ProjectId) {
    let projects = Arc::new(FakeProjectRepo::new());
    let project_a = projects
        .upsert(Path::new("/a"), Some("a"), None)
        .expect("seed project a")
        .id;
    let project_b = projects
        .upsert(Path::new("/b"), Some("b"), None)
        .expect("seed project b")
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
    (facade, project_a, project_b)
}

fn todo_doc(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: format!("do {title}"),
        status: TodoStatus::Open,
    }
}

fn scratchpad_body() -> String {
    "## Objective\nship it\n\n## Status\nactive".to_owned()
}

fn work_of(work: &ProjectWork, id: TodoId) -> &TodoWork {
    work.todos
        .iter()
        .find(|todo| todo.id == id)
        .unwrap_or_else(|| panic!("todo {id:?} is present"))
}

#[test]
fn names_the_role_a_bound_session_earned_on_each_document() {
    let (facade, project) = facade();
    let read_todo = facade
        .todo_create_in(project, todo_doc("read me"), None)
        .expect("seed")
        .id;
    let write_todo = facade
        .todo_create_in(project, todo_doc("write me"), None)
        .expect("seed")
        .id;
    let lock_todo = facade
        .todo_create_in(project, todo_doc("lock me"), None)
        .expect("seed")
        .id;
    let (process, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);

    facade.scoped(session).todo_get(read_todo).expect("read");
    facade
        .scoped(session)
        .todo_add_tag(write_todo, "urgent")
        .expect("write");
    facade.scoped(session).todo_lock(lock_todo).expect("lock");

    let work = facade.project_work(project).expect("query");
    assert_eq!(
        work_of(&work, read_todo).participants,
        vec![DocumentParticipant {
            process,
            label: "lead".into(),
            role: DocumentRole::Reading,
        }]
    );
    assert_eq!(
        work_of(&work, write_todo).participants,
        vec![DocumentParticipant {
            process,
            label: "lead".into(),
            role: DocumentRole::Editing,
        }]
    );
    assert_eq!(
        work_of(&work, lock_todo).participants,
        vec![DocumentParticipant {
            process,
            label: "lead".into(),
            role: DocumentRole::Implementing,
        }]
    );
}

#[test]
fn omits_a_todo_the_owner_marked_done() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("ship"), None)
        .expect("seed")
        .id;
    let (_process, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    facade.scoped(session).todo_lock(todo).expect("lock");

    facade.scoped(session).todo_complete(todo).expect("done");

    let work = facade.project_work(project).expect("query");
    assert!(
        work.todos.iter().all(|entry| entry.id != todo),
        "a done todo never appears, however it was touched"
    );
}

#[test]
fn omits_a_document_touched_only_by_a_process_outside_the_registry() {
    let (facade, project) = facade();
    let touched = facade
        .todo_create_in(project, todo_doc("touched"), None)
        .expect("seed")
        .id;
    let locked = facade
        .todo_create_in(project, todo_doc("locked"), None)
        .expect("seed")
        .id;
    let ghost = ProcessId::from_raw(999_999);

    facade
        .session_activity
        .record_todo(ghost, touched, AccessKind::Loaded);
    facade
        .todos
        .lock(project, locked, ghost)
        .expect("lock through the store directly")
        .expect("the todo exists");

    let work = facade.project_work(project).expect("query");
    assert!(
        work.todos.iter().all(|entry| entry.id != touched),
        "a recorded access from a process outside the registry never appears"
    );
    assert!(
        work.todos.iter().all(|entry| entry.id != locked),
        "a lock held by a process outside the registry never appears"
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
    facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let session = facade.open_session(PeerCredentials::unauthenticated());

    facade
        .scoped(session)
        .todo_get(todo)
        .expect("an unbound caller may still read within its resolved scope");

    let work = facade.project_work(project).expect("query");
    assert!(
        work.todos.iter().all(|entry| entry.id != todo),
        "an unbound caller's read is attributed to no process, so it never earns the todo a row"
    );
}

#[test]
fn an_external_caller_records_nothing() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("open"), None)
        .expect("seed")
        .id;
    facade
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

    let work = facade.project_work(project).expect("query");
    assert!(
        work.todos.iter().all(|entry| entry.id != todo),
        "an external caller's read is attributed to no process, so it never earns the todo a row"
    );
}

#[test]
fn lists_one_document_once_with_every_live_participant() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("ship"), None)
        .expect("seed")
        .id;
    let (holder, holder_session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    let (reader, reader_session) = bound_agent(&facade, project, "worker", TEST_PEER_PGID + 1);

    facade.scoped(holder_session).todo_lock(todo).expect("lock");
    facade.scoped(reader_session).todo_get(todo).expect("read");

    let work = facade.project_work(project).expect("query");
    let entry = work_of(&work, todo);
    assert_eq!(
        entry.participants,
        vec![
            DocumentParticipant {
                process: holder,
                label: "lead".into(),
                role: DocumentRole::Implementing,
            },
            DocumentParticipant {
                process: reader,
                label: "worker".into(),
                role: DocumentRole::Reading,
            },
        ],
        "one row, both participants, ordered by ascending process id"
    );
}

#[test]
fn reports_a_lock_holder_that_recorded_no_tool_access() {
    let (facade, project) = facade();
    let todo = facade
        .todo_create_in(project, todo_doc("ship"), None)
        .expect("seed")
        .id;
    let (process, _session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);
    // Sets `locked_by` directly, bypassing the recording helpers `ScopedFacade::todo_lock` calls.
    facade
        .todos
        .lock(project, todo, process)
        .expect("lock through the store directly")
        .expect("the todo exists");

    let work = facade.project_work(project).expect("query");
    assert_eq!(
        work_of(&work, todo).participants,
        vec![DocumentParticipant {
            process,
            label: "lead".into(),
            role: DocumentRole::Implementing,
        }],
        "a held lock is live work even with nothing recorded through a tool"
    );
}

#[test]
fn keeps_documents_of_other_projects_out() {
    let (facade, project_a, project_b) = facade_with_two_projects();
    let todo_a = facade
        .todo_create_in(project_a, todo_doc("in a"), None)
        .expect("seed")
        .id;
    let todo_b = facade
        .todo_create_in(project_b, todo_doc("in b"), None)
        .expect("seed")
        .id;
    let (_agent_a, session_a) = bound_agent(&facade, project_a, "lead-a", TEST_PEER_PGID);
    let (_agent_b, session_b) = bound_agent(&facade, project_b, "lead-b", TEST_PEER_PGID + 1);
    facade.scoped(session_a).todo_get(todo_a).expect("read a");
    facade.scoped(session_b).todo_get(todo_b).expect("read b");

    let work_a = facade.project_work(project_a).expect("query a");
    let work_b = facade.project_work(project_b).expect("query b");

    assert_eq!(
        work_a.todos.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![todo_a]
    );
    assert_eq!(
        work_b.todos.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![todo_b]
    );
}

#[test]
fn reports_a_scratchpad_write_as_editing_and_a_read_as_reading() {
    let (facade, project) = facade();
    let read_pad = facade
        .scratchpad_write_in(project, "read-me", scratchpad_body(), None)
        .expect("seed")
        .id;
    let write_pad = facade
        .scratchpad_write_in(project, "write-me", scratchpad_body(), None)
        .expect("seed");
    let (process, session) = bound_agent(&facade, project, "lead", TEST_PEER_PGID);

    facade
        .scoped(session)
        .scratchpad_read("read-me")
        .expect("read");
    facade
        .scoped(session)
        .scratchpad_write("write-me", scratchpad_body(), Some(write_pad.revision))
        .expect("write");

    let work = facade.project_work(project).expect("query");
    let read_entry = work
        .scratchpads
        .iter()
        .find(|entry| entry.id == read_pad)
        .expect("the read scratchpad is present");
    assert_eq!(
        read_entry.participants,
        vec![DocumentParticipant {
            process,
            label: "lead".into(),
            role: DocumentRole::Reading,
        }]
    );
    let write_entry = work
        .scratchpads
        .iter()
        .find(|entry| entry.id == write_pad.id)
        .expect("the written scratchpad is present");
    assert_eq!(
        write_entry.participants,
        vec![DocumentParticipant {
            process,
            label: "lead".into(),
            role: DocumentRole::Editing,
        }]
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
