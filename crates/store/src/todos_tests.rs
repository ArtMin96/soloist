use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

use soloist_core::{
    CommentAuthor, ProcessId, ProjectId, ProjectRepo, StoredTodo, TodoDoc, TodoRepo, TodoStatus,
    TodoWriteResult,
};
use tempfile::tempdir;

use super::*;

fn project(store: &SqliteStore, root: &str) -> ProjectId {
    store
        .upsert(Path::new(root), None, None)
        .expect("project for todo fk")
        .id
}

fn doc(title: &str, status: TodoStatus) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: "do it".into(),
        status,
    }
}

fn written(result: TodoWriteResult) -> StoredTodo {
    match result {
        TodoWriteResult::Written(stored) => *stored,
        other => panic!("expected a write, got {other:?}"),
    }
}

#[test]
fn create_then_read_round_trips_every_column_through_json() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");

    let created = store
        .create(project, &doc("ship", TodoStatus::Open))
        .expect("create");
    assert_eq!(created.revision, 1);
    assert!(created.id.get() > 0, "the store assigns a durable id");

    // Exercise every live column, then prove it all survives the JSON round-trip.
    store.add_tag(project, created.id, "ui").expect("tag");
    let blocker = store
        .create(project, &doc("dep", TodoStatus::Open))
        .unwrap();
    store
        .set_blockers(project, created.id, &[blocker.id])
        .expect("blockers");
    let author = CommentAuthor::Process {
        id: ProcessId::from_raw(3),
        label: "Web".into(),
    };
    store
        .comment_create(project, created.id, "looks good", Some(author.clone()))
        .expect("comment");
    TodoRepo::lock(&store, project, created.id, ProcessId::from_raw(7)).expect("lock");

    let read = store
        .read(project, created.id)
        .expect("read")
        .expect("exists");
    assert_eq!(read.doc, doc("ship", TodoStatus::Open));
    assert_eq!(read.tags, vec!["ui".to_string()]);
    assert_eq!(read.blockers, vec![blocker.id]);
    assert_eq!(read.comments.len(), 1);
    assert_eq!(read.comments[0].body, "looks good");
    assert_eq!(
        read.comments[0].author,
        Some(author),
        "the comment author survives the JSON round-trip"
    );
    assert_eq!(read.locked_by, Some(ProcessId::from_raw(7)));
}

#[test]
fn write_doc_is_revision_guarded() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let todo = store.create(project, &doc("v1", TodoStatus::Open)).unwrap();

    // Current revision applies and bumps.
    let updated = written(
        store
            .write_doc(
                project,
                todo.id,
                &doc("v2", TodoStatus::InProgress),
                Some(1),
            )
            .expect("update"),
    );
    assert_eq!(updated.revision, 2);

    // Stale revision is refused, nothing changes.
    assert!(matches!(
        store.write_doc(project, todo.id, &doc("v3", TodoStatus::Done), Some(1)),
        Ok(TodoWriteResult::Conflict { actual: 2 })
    ));

    // A missing todo is NotFound.
    assert!(matches!(
        store.write_doc(
            project,
            TodoId::from_raw(9999),
            &doc("x", TodoStatus::Open),
            None
        ),
        Ok(TodoWriteResult::NotFound)
    ));
}

#[test]
fn unmet_blockers_skips_done_and_deleted_blockers() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let open = store
        .create(project, &doc("open", TodoStatus::Open))
        .unwrap();
    let done = store
        .create(project, &doc("done", TodoStatus::Done))
        .unwrap();
    let gone = store
        .create(project, &doc("gone", TodoStatus::Open))
        .unwrap();
    assert!(store.delete(project, gone.id).unwrap());

    let unmet = store
        .unmet_blockers(project, &[open.id, done.id, gone.id])
        .expect("unmet");
    // Only the existing, not-done blocker is unmet; the done and the deleted are met.
    assert_eq!(unmet, vec![open.id]);
}

#[test]
fn a_lock_is_a_signal_and_releases_by_owner_and_on_reconcile() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let todo = store.create(project, &doc("x", TodoStatus::Open)).unwrap();
    let alice = ProcessId::from_raw(10);
    let bob = ProcessId::from_raw(20);

    assert_eq!(
        TodoRepo::lock(&store, project, todo.id, alice)
            .unwrap()
            .unwrap()
            .locked_by,
        Some(alice)
    );
    // Bob cannot steal it.
    assert_eq!(
        TodoRepo::lock(&store, project, todo.id, bob)
            .unwrap()
            .unwrap()
            .locked_by,
        Some(alice)
    );
    // The owner closing releases exactly its own lock.
    assert_eq!(store.release_owner(alice).expect("release owner"), 1);
    assert_eq!(
        store.read(project, todo.id).unwrap().unwrap().locked_by,
        None
    );

    // Re-lock, then a launch reconcile clears it while keeping the todo.
    TodoRepo::lock(&store, project, todo.id, bob).unwrap();
    assert_eq!(store.clear_locks().expect("clear locks"), 1);
    let after = store
        .read(project, todo.id)
        .unwrap()
        .expect("todo survives");
    assert_eq!(after.locked_by, None);
}

#[test]
fn deleting_a_project_cascades_to_its_todos() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let todo = store.create(project, &doc("x", TodoStatus::Open)).unwrap();

    store.remove(project).expect("remove project");
    assert!(
        store.read(project, todo.id).expect("read").is_none(),
        "the FK cascade drops the project's todos"
    );
}

#[test]
fn concurrent_doc_writes_at_one_revision_apply_exactly_one() {
    let store = Arc::new(SqliteStore::open_in_memory().expect("open"));
    let project = project(&store, "/p/app");
    let todo = store.create(project, &doc("v1", TodoStatus::Open)).unwrap();

    const WRITERS: usize = 16;
    let barrier = Arc::new(Barrier::new(WRITERS));
    let wins = Arc::new(AtomicUsize::new(0));
    let conflicts = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..WRITERS)
        .map(|_| {
            let store = store.clone();
            let barrier = barrier.clone();
            let wins = wins.clone();
            let conflicts = conflicts.clone();
            let id = todo.id;
            std::thread::spawn(move || {
                barrier.wait();
                match store.write_doc(project, id, &doc("v2", TodoStatus::InProgress), Some(1)) {
                    Ok(TodoWriteResult::Written(_)) => wins.fetch_add(1, Ordering::Relaxed),
                    Ok(TodoWriteResult::Conflict { .. }) => {
                        conflicts.fetch_add(1, Ordering::Relaxed)
                    }
                    other => panic!("unexpected write outcome: {other:?}"),
                };
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("writer thread");
    }

    // The revision guard is atomic: exactly one writer applies (rev 1 → 2), the rest are refused.
    assert_eq!(wins.load(Ordering::Relaxed), 1);
    assert_eq!(conflicts.load(Ordering::Relaxed), WRITERS - 1);
    assert_eq!(store.read(project, todo.id).unwrap().unwrap().revision, 2);
}

#[test]
fn durable_todos_survive_a_reopen() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let id = {
        let store = SqliteStore::open(&db).expect("open");
        let project = project(&store, "/p/app");
        store
            .create(project, &doc("persist", TodoStatus::Open))
            .unwrap()
            .id
    };

    // Reopen: the project row persists, so re-deriving its id returns the same one, and the todo
    // (durable content, G11) is still there.
    let store = SqliteStore::open(&db).expect("reopen");
    let project = project(&store, "/p/app");
    let found = store
        .read(project, id)
        .expect("read")
        .expect("the todo persisted across the reopen");
    assert_eq!(found.doc.title, "persist");
}

#[test]
fn transfer_moves_the_todo_clearing_blockers_and_lock_but_keeping_comments_and_doc() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    let todo = store
        .create(a, &doc("ship", TodoStatus::InProgress))
        .expect("create");
    let blocker = store
        .create(a, &doc("dep", TodoStatus::Open))
        .expect("blocker");
    store.add_blocker(a, todo.id, blocker.id).expect("block");
    store
        .comment_create(
            a,
            todo.id,
            "note",
            Some(CommentAuthor::External { label: "x".into() }),
        )
        .expect("comment");
    // Fully-qualified: `SqliteStore::lock` (the connection guard) shadows the `TodoRepo::lock` verb.
    TodoRepo::lock(&store, a, todo.id, ProcessId::from_raw(7)).expect("lock");

    let moved = store
        .transfer(a, b, todo.id)
        .expect("transfer")
        .expect("moved");
    assert_eq!(moved.id, todo.id, "durable id kept");
    assert_eq!(moved.project, b, "now under the target project");
    assert_eq!(moved.doc.status, TodoStatus::InProgress, "document kept");
    assert_eq!(moved.comments.len(), 1, "comments kept");
    assert!(moved.blockers.is_empty(), "blockers cleared");
    assert_eq!(moved.locked_by, None, "lock cleared");
    assert!(
        store.read(a, todo.id).expect("read a").is_none(),
        "gone from A"
    );
    assert!(
        store.read(b, todo.id).expect("read b").is_some(),
        "present in B"
    );
}
