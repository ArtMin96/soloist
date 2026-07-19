use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

use soloist_core::{
    CommentAuthor, ProcessId, ProjectId, ProjectRepo, ScratchpadId, ScratchpadLink, StoredTodo,
    TodoDoc, TodoRepo, TodoStatus, TodoWriteResult, WriteResult,
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
        .create(project, &doc("ship", TodoStatus::Open), None)
        .expect("create");
    assert_eq!(created.revision, 1);
    assert!(created.id.get() > 0, "the store assigns a durable id");

    // Exercise every live column, then prove it all survives the JSON round-trip.
    store.add_tag(project, created.id, "ui").expect("tag");
    let blocker = store
        .create(project, &doc("dep", TodoStatus::Open), None)
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
    let todo = store
        .create(project, &doc("v1", TodoStatus::Open), None)
        .unwrap();

    // Current revision applies and bumps.
    let updated = written(
        store
            .write_doc(
                project,
                todo.id,
                &doc("v2", TodoStatus::InProgress),
                ScratchpadLink::Unchanged,
                Some(1),
            )
            .expect("update"),
    );
    assert_eq!(updated.revision, 2);

    // Stale revision is refused, nothing changes.
    assert!(matches!(
        store.write_doc(
            project,
            todo.id,
            &doc("v3", TodoStatus::Done),
            ScratchpadLink::Unchanged,
            Some(1)
        ),
        Ok(TodoWriteResult::Conflict { actual: 2 })
    ));

    // A missing todo is NotFound.
    assert!(matches!(
        store.write_doc(
            project,
            TodoId::from_raw(9999),
            &doc("x", TodoStatus::Open),
            ScratchpadLink::Unchanged,
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
        .create(project, &doc("open", TodoStatus::Open), None)
        .unwrap();
    let done = store
        .create(project, &doc("done", TodoStatus::Done), None)
        .unwrap();
    let gone = store
        .create(project, &doc("gone", TodoStatus::Open), None)
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
    let todo = store
        .create(project, &doc("x", TodoStatus::Open), None)
        .unwrap();
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
    let todo = store
        .create(project, &doc("x", TodoStatus::Open), None)
        .unwrap();

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
    let todo = store
        .create(project, &doc("v1", TodoStatus::Open), None)
        .unwrap();

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
                match store.write_doc(
                    project,
                    id,
                    &doc("v2", TodoStatus::InProgress),
                    ScratchpadLink::Unchanged,
                    Some(1),
                ) {
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
            .create(project, &doc("persist", TodoStatus::Open), None)
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
        .create(a, &doc("ship", TodoStatus::InProgress), None)
        .expect("create");
    let blocker = store
        .create(a, &doc("dep", TodoStatus::Open), None)
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

// The scratchpad port is reached through fully qualified calls rather than a `use`, because
// `SqliteStore` implements both repositories and their `read`/`delete` names would collide.
/// Writes a scratchpad in `project` and returns its durable id — the handle a todo associates with.
fn scratchpad(store: &SqliteStore, project: ProjectId, name: &str) -> ScratchpadId {
    match <SqliteStore as soloist_core::ScratchpadRepo>::write(
        store, project, name, "the plan", None, 0,
    )
    .expect("write a scratchpad")
    {
        WriteResult::Written(stored) => stored.id,
        other => panic!("expected a write, got {other:?}"),
    }
}

#[test]
fn an_association_round_trips_with_the_scratchpads_current_handle() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let pad = scratchpad(&store, project, "release-plan");

    let created = store
        .create(project, &doc("ship", TodoStatus::Open), Some(pad))
        .expect("create linked");
    let read = store
        .read(project, created.id)
        .expect("read")
        .expect("the todo exists");

    let link = read
        .scratchpad
        .expect("the association survives the round trip");
    assert_eq!(link.id, pad);
    assert_eq!(link.name, "release-plan");
    // Only the id is persisted, so a rename follows through to the projected handle.
    <SqliteStore as soloist_core::ScratchpadRepo>::rename(
        &store,
        project,
        "release-plan",
        "rollout-plan",
    )
    .expect("rename the scratchpad");
    let renamed = store
        .read(project, created.id)
        .expect("read")
        .expect("the todo exists")
        .scratchpad
        .expect("still linked");
    assert_eq!(renamed.id, pad);
    assert_eq!(renamed.name, "rollout-plan");
}

#[test]
fn an_unlinked_todo_round_trips_with_a_null_column() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");

    let created = store
        .create(project, &doc("ship", TodoStatus::Open), None)
        .expect("create unlinked");
    assert_eq!(created.scratchpad, None);

    let raw: Option<i64> = store
        .lock()
        .query_row(
            "SELECT scratchpad_id FROM todos WHERE id = ?1",
            [created.id.get() as i64],
            |row| row.get(0),
        )
        .expect("read the raw column");
    assert_eq!(raw, None, "an unlinked todo stores NULL, not a sentinel");

    let listed = TodoRepo::list(&store, project).expect("list");
    assert_eq!(
        listed.len(),
        1,
        "the outer join keeps an unlinked todo visible"
    );
    assert_eq!(listed[0].scratchpad, None);
}

#[test]
fn a_doc_write_applies_the_stated_link_and_leaves_an_unchanged_one_alone() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let pad = scratchpad(&store, project, "release-plan");
    let other = scratchpad(&store, project, "rollout-plan");
    let created = store
        .create(project, &doc("ship", TodoStatus::Open), Some(pad))
        .expect("create linked");

    let untouched = written(
        store
            .write_doc(
                project,
                created.id,
                &doc("ship it", TodoStatus::InProgress),
                ScratchpadLink::Unchanged,
                Some(created.revision),
            )
            .expect("write saying nothing about the link"),
    );
    assert_eq!(untouched.scratchpad.map(|link| link.id), Some(pad));

    let relinked = written(
        store
            .write_doc(
                project,
                created.id,
                &doc("ship it", TodoStatus::InProgress),
                ScratchpadLink::Linked(other),
                Some(untouched.revision),
            )
            .expect("write relinking"),
    );
    assert_eq!(relinked.scratchpad.map(|link| link.id), Some(other));

    let cleared = written(
        store
            .write_doc(
                project,
                created.id,
                &doc("ship it", TodoStatus::InProgress),
                ScratchpadLink::Cleared,
                Some(relinked.revision),
            )
            .expect("write clearing"),
    );
    assert_eq!(cleared.scratchpad, None);
}

#[test]
fn deleting_a_scratchpad_unlinks_the_todos_that_referenced_it() {
    // The foreign key carries this, so a todo can never be left pointing at a document that is
    // gone — not even by a writer that never learns about the delete.
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let pad = scratchpad(&store, project, "release-plan");
    let created = store
        .create(project, &doc("ship", TodoStatus::Open), Some(pad))
        .expect("create linked");

    assert!(
        <SqliteStore as soloist_core::ScratchpadRepo>::delete(&store, project, "release-plan")
            .expect("delete the scratchpad")
    );

    let after = store
        .read(project, created.id)
        .expect("read")
        .expect("the todo itself survives its scratchpad");
    assert_eq!(after.scratchpad, None);
    assert_eq!(after.doc, created.doc);
    let raw: Option<i64> = store
        .lock()
        .query_row(
            "SELECT scratchpad_id FROM todos WHERE id = ?1",
            [created.id.get() as i64],
            |row| row.get(0),
        )
        .expect("read the raw column");
    assert_eq!(raw, None, "the column is nulled, not left dangling");
}

#[test]
fn transferring_a_todo_clears_its_scratchpad_association() {
    let store = SqliteStore::open_in_memory().expect("open");
    let from = project(&store, "/p/app");
    let to = project(&store, "/p/other");
    let pad = scratchpad(&store, from, "release-plan");
    let created = store
        .create(from, &doc("ship", TodoStatus::Open), Some(pad))
        .expect("create linked");

    let moved = store
        .transfer(from, to, created.id)
        .expect("transfer")
        .expect("the todo moved");

    assert_eq!(moved.scratchpad, None);
    assert_eq!(moved.doc, created.doc);
}
