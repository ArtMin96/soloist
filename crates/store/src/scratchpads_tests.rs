use std::path::Path;
use std::sync::{Arc, Barrier};

use soloist_core::{
    ProcessId, ProjectId, ProjectRepo, RenameResult, ScratchpadId, ScratchpadRepo, StoreError,
    StoredScratchpad, StoredTodo, TodoDoc, TodoId, TodoStatus, TransferResult,
    TransferredScratchpad, WriteResult,
};
use tempfile::tempdir;

use super::*;

/// A fixed wall clock for the writes whose recency is not under test — these exercise revision,
/// rename, tag, archive, and cascade semantics, and `updated_at` is verified on its own below.
const FIXED_NOW: u64 = 1_700_000_000_000;

/// Writes at [`FIXED_NOW`], so the semantics tests read the same as before `updated_at` existed. The
/// recency-stamping behaviour has its own test that drives real timestamps.
trait WriteAt {
    fn write_at(
        &self,
        project: ProjectId,
        name: &str,
        body: &str,
        expected: Option<u64>,
    ) -> Result<WriteResult, StoreError>;
}

impl WriteAt for SqliteStore {
    fn write_at(
        &self,
        project: ProjectId,
        name: &str,
        body: &str,
        expected: Option<u64>,
    ) -> Result<WriteResult, StoreError> {
        self.write(project, name, body, expected, FIXED_NOW)
    }
}

fn project(store: &SqliteStore, root: &str) -> ProjectId {
    store
        .upsert(Path::new(root), None, None)
        .expect("project for scratchpad fk")
        .id
}

/// A representative Markdown body carrying `marker` so writes can be told apart.
fn body(marker: &str) -> String {
    format!("## Objective\nShip v1\n\n## Status\n{marker}")
}

fn written(result: WriteResult) -> StoredScratchpad {
    match result {
        WriteResult::Written(stored) => *stored,
        WriteResult::Conflict { actual } => {
            panic!("expected a write, got a conflict at {actual:?}")
        }
    }
}

fn transferred(result: TransferResult) -> TransferredScratchpad {
    match result {
        TransferResult::Transferred(moved) => *moved,
        other => panic!("expected a transfer, got {other:?}"),
    }
}

#[test]
fn create_then_read_round_trips_the_body() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");

    let created = written(
        store
            .write_at(project, "plan", &body("started"), None)
            .expect("create"),
    );
    assert_eq!(created.revision, 1);
    assert!(created.id.get() > 0, "the store assigns a durable id");

    let read = store.read(project, "plan").expect("read").expect("exists");
    // The Markdown body survives the store round-trip verbatim.
    assert_eq!(read.body, body("started"));
    assert_eq!(read, created);
}

#[test]
fn a_write_is_revision_guarded() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    store
        .write_at(project, "plan", &body("a"), None)
        .expect("create");

    // Update at the current revision bumps it.
    let updated = written(
        store
            .write_at(project, "plan", &body("b"), Some(1))
            .expect("update"),
    );
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.body, body("b"));

    // A stale revision conflicts and changes nothing.
    assert_eq!(
        store
            .write_at(project, "plan", &body("c"), Some(1))
            .expect("stale"),
        WriteResult::Conflict { actual: Some(2) }
    );
    assert_eq!(
        store.read(project, "plan").unwrap().unwrap().body,
        body("b")
    );

    // Creating over an existing name conflicts.
    assert_eq!(
        store
            .write_at(project, "plan", &body("d"), None)
            .expect("recreate"),
        WriteResult::Conflict { actual: Some(2) }
    );

    // Updating a missing scratchpad conflicts with no record.
    assert_eq!(
        store
            .write_at(project, "absent", &body("e"), Some(5))
            .expect("update missing"),
        WriteResult::Conflict { actual: None }
    );
}

#[test]
fn rename_keeps_the_id_and_enforces_uniqueness() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let created = written(
        store
            .write_at(project, "old", &body("a"), None)
            .expect("create"),
    );
    store
        .write_at(project, "taken", &body("a"), None)
        .expect("create taken");

    let renamed = match store.rename(project, "old", "new").expect("rename") {
        RenameResult::Renamed(stored) => *stored,
        other => panic!("expected a rename, got {other:?}"),
    };
    assert_eq!(renamed.name, "new");
    assert_eq!(
        renamed.id, created.id,
        "the durable id is stable across a rename"
    );

    assert_eq!(
        store
            .rename(project, "missing", "x")
            .expect("rename missing"),
        RenameResult::NotFound
    );
    assert_eq!(
        store
            .rename(project, "new", "taken")
            .expect("rename onto taken"),
        RenameResult::NameTaken
    );
}

#[test]
fn tags_add_dedupe_remove_and_list_distinct() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    store
        .write_at(project, "a", &body("a"), None)
        .expect("create a");
    store
        .write_at(project, "b", &body("a"), None)
        .expect("create b");

    let tagged = store
        .add_tags(
            project,
            "a",
            &["release".into(), "release".into(), "p1".into()],
        )
        .expect("add")
        .expect("exists");
    assert_eq!(tagged.tags, vec!["p1".to_string(), "release".to_string()]);

    store.add_tags(project, "b", &["p1".into()]).expect("tag b");
    assert_eq!(
        store.tags(project).expect("tags"),
        vec!["p1".to_string(), "release".to_string()]
    );

    let untagged = store
        .remove_tags(project, "a", &["release".into()])
        .expect("remove")
        .expect("exists");
    assert_eq!(untagged.tags, vec!["p1".to_string()]);

    assert!(store
        .add_tags(project, "missing", &["x".into()])
        .expect("add")
        .is_none());
}

#[test]
fn archive_is_a_flag_and_delete_removes() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    store
        .write_at(project, "a", &body("a"), None)
        .expect("create");

    let archived = store
        .set_archived(project, "a", true)
        .expect("archive")
        .expect("exists");
    assert!(archived.archived);
    assert!(
        store.read(project, "a").unwrap().is_some(),
        "archive keeps the document"
    );

    assert!(store.delete(project, "a").expect("delete"));
    assert!(!store.delete(project, "a").expect("second delete"));
    assert!(store.read(project, "a").unwrap().is_none());
}

#[test]
fn list_is_scoped_and_ordered_by_name() {
    let store = SqliteStore::open_in_memory().expect("open");
    let one = project(&store, "/p/one");
    let two = project(&store, "/p/two");
    store
        .write_at(one, "zebra", &body("a"), None)
        .expect("create");
    store
        .write_at(one, "alpha", &body("a"), None)
        .expect("create");
    store
        .write_at(two, "other", &body("a"), None)
        .expect("create");

    let names: Vec<String> = ScratchpadRepo::list(&store, one)
        .expect("list")
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, vec!["alpha".to_string(), "zebra".to_string()]);
}

#[test]
fn scratchpads_survive_a_store_reopen() {
    // Coordination content persists across an app restart: unlike leases and timers, scratchpads
    // are durable and not cleared on launch.
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let id = {
        let store = SqliteStore::open(&db).expect("open");
        let project = project(&store, "/p/app");
        let created = written(
            store
                .write_at(project, "plan", &body("started"), None)
                .expect("create"),
        );
        store
            .add_tags(project, "plan", &["release".into()])
            .expect("tag");
        created.id
    };

    // A fresh process opens the same database: the scratchpad is still there, with its id, body,
    // and tags intact.
    let store = SqliteStore::open(&db).expect("reopen");
    let reopened = store
        .read(ProjectId::from_raw(1), "plan")
        .expect("read")
        .expect("the scratchpad survives the reopen");
    assert_eq!(reopened.id, id);
    assert_eq!(reopened.body, body("started"));
    assert_eq!(reopened.tags, vec!["release".to_string()]);
}

#[test]
fn deleting_a_project_cascades_to_its_scratchpads() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    store
        .write_at(project, "plan", &body("a"), None)
        .expect("create");

    store.remove(project).expect("remove project");
    assert!(
        store.read(project, "plan").expect("read").is_none(),
        "the project's scratchpads are dropped with it"
    );
}

#[test]
fn concurrent_writes_at_one_revision_apply_exactly_one() {
    // The race the atomic revision guard fixes: many agents update one scratchpad from the same
    // revision at once. Exactly one write must apply (bumping the revision); every other must be
    // refused as a conflict — never two writes accepted at one revision.
    let dir = tempdir().expect("temp dir");
    let store = Arc::new(SqliteStore::open(&dir.path().join("soloist.db")).expect("open"));
    let project = project(&store, "/p/race");
    store
        .write_at(project, "plan", &body("base"), None)
        .expect("create at revision 1");
    const CONTENDERS: u64 = 16;

    let barrier = Arc::new(Barrier::new(CONTENDERS as usize));
    let outcomes: Vec<WriteResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CONTENDERS)
            .map(|n| {
                let store = store.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    store
                        .write_at(project, "plan", &body(&format!("edit-{n}")), Some(1))
                        .expect("write")
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("thread"))
            .collect()
    });

    let applied = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, WriteResult::Written(_)))
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, WriteResult::Conflict { actual: Some(2) }))
        .count();
    assert_eq!(applied, 1, "exactly one write at revision 1 applies");
    assert_eq!(
        conflicts,
        (CONTENDERS - 1) as usize,
        "every other writer is refused against the single bumped revision"
    );
    // The scratchpad advanced exactly one revision — no lost update, no double-apply.
    assert_eq!(store.read(project, "plan").unwrap().unwrap().revision, 2);
}

/// A minimal valid todo document.
fn todo_doc(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.into(),
        body: "do it".into(),
        status: TodoStatus::Open,
    }
}

/// Creates a todo in `project` derived from `scratchpad`, returning its id.
fn derived(
    store: &SqliteStore,
    project: ProjectId,
    title: &str,
    scratchpad: ScratchpadId,
) -> TodoId {
    soloist_core::TodoRepo::create(store, project, &todo_doc(title), Some(scratchpad))
        .expect("create a derived todo")
        .id
}

/// The todo `id` as `project` reads it, or `None` when it is not in that project.
fn todo_in(store: &SqliteStore, project: ProjectId, id: TodoId) -> Option<StoredTodo> {
    soloist_core::TodoRepo::read(store, project, id).expect("read a todo")
}

#[test]
fn transfer_takes_the_derived_todos_along_and_keeps_their_association() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    let plan = written(
        store
            .write_at(a, "plan", &body("draft"), None)
            .expect("plan"),
    );
    let notes = written(
        store
            .write_at(a, "notes", &body("aside"), None)
            .expect("notes"),
    );
    let ship = derived(&store, a, "ship", plan.id);
    let test = derived(&store, a, "test", plan.id);
    // Neither of these derives from `plan`, so neither may move.
    let tidy = derived(&store, a, "tidy", notes.id);
    let loose = soloist_core::TodoRepo::create(&store, a, &todo_doc("triage"), None)
        .expect("an unlinked todo")
        .id;

    let moved = transferred(store.transfer(a, "plan", b).expect("transfer"));

    assert_eq!(moved.todos, vec![ship, test], "both derived todos moved");
    for id in [ship, test] {
        let after = todo_in(&store, b, id).expect("now reads from the target project");
        let link = after
            .scratchpad
            .expect("the association is kept — both ends moved, so it still resolves");
        assert_eq!(link.id, plan.id, "still derived from the same document");
        assert_eq!(link.name, "plan", "and the handle still resolves");
        assert!(todo_in(&store, a, id).is_none(), "gone from the source");
    }
    for (id, pad) in [(tidy, Some(notes.id)), (loose, None)] {
        let after = todo_in(&store, a, id).expect("untouched in the source project");
        assert_eq!(after.scratchpad.map(|link| link.id), pad);
    }
}

#[test]
fn transfer_clears_blockers_naming_todos_left_behind_and_keeps_those_that_move() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    let plan = written(
        store
            .write_at(a, "plan", &body("draft"), None)
            .expect("plan"),
    );
    let ship = derived(&store, a, "ship", plan.id);
    let test = derived(&store, a, "test", plan.id);
    let stays = soloist_core::TodoRepo::create(&store, a, &todo_doc("unrelated"), None)
        .expect("a todo that stays behind")
        .id;
    // `ship` waits on one todo that moves with it and one that does not.
    soloist_core::TodoRepo::set_blockers(&store, a, ship, &[test, stays]).expect("set blockers");

    store.transfer(a, "plan", b).expect("transfer");

    let after = todo_in(&store, b, ship).expect("moved");
    assert_eq!(
        after.blockers,
        vec![test],
        "the blocker that moved too survives; the one left behind names another project's row"
    );
}

#[test]
fn transfer_drops_the_process_owned_locks_on_the_todos_it_moves() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    let plan = written(
        store
            .write_at(a, "plan", &body("draft"), None)
            .expect("plan"),
    );
    let ship = derived(&store, a, "ship", plan.id);
    soloist_core::TodoRepo::lock(&store, a, ship, ProcessId::from_raw(7)).expect("lock it");

    store.transfer(a, "plan", b).expect("transfer");

    assert_eq!(
        todo_in(&store, b, ship).expect("moved").locked_by,
        None,
        "a per-run process-owned lock does not follow the todo across projects"
    );
}

#[test]
fn a_cascade_that_fails_part_way_leaves_the_scratchpad_and_every_todo_where_they_were() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    let plan = written(
        store
            .write_at(a, "plan", &body("draft"), None)
            .expect("plan"),
    );
    let first = derived(&store, a, "ship", plan.id);
    let second = derived(&store, a, "test", plan.id);
    // Corrupt the *second* todo's blockers, so the cascade fails only after the scratchpad and the
    // first todo have already been written inside the transaction.
    store
        .lock()
        .execute(
            "UPDATE todos SET blockers = 'not json' WHERE id = ?1",
            [second.get() as i64],
        )
        .expect("corrupt one row");

    assert!(
        store.transfer(a, "plan", b).is_err(),
        "the undecodable row fails the cascade"
    );

    assert!(
        store.read(b, "plan").expect("read b").is_none(),
        "the scratchpad's move was rolled back"
    );
    assert!(
        store.read(a, "plan").expect("read a").is_some(),
        "it is still in the project it started in"
    );
    assert!(
        todo_in(&store, b, first).is_none(),
        "the todo that had already moved was rolled back with it"
    );
    assert!(
        todo_in(&store, a, first).is_some(),
        "and still reads from the source project"
    );
}

#[test]
fn transfer_moves_the_scratchpad_keeping_identity_and_refuses_a_taken_name() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    let created = match store
        .write_at(a, "plan", &body("draft"), None)
        .expect("create")
    {
        WriteResult::Written(stored) => *stored,
        other => panic!("expected a write, got {other:?}"),
    };

    // A name already used in the target is refused.
    store
        .write_at(b, "plan", &body("draft"), None)
        .expect("create in B");
    assert!(matches!(
        store.transfer(a, "plan", b).expect("transfer"),
        TransferResult::NameTaken
    ));

    // Clear the collision, then the move keeps the durable id and revision.
    store.delete(b, "plan").expect("delete B copy");
    let moved = transferred(store.transfer(a, "plan", b).expect("transfer")).scratchpad;
    assert_eq!(moved.id, created.id, "durable id kept");
    assert_eq!(moved.project, b, "now under the target project");
    assert_eq!(moved.revision, created.revision, "revision kept");
    assert!(
        store.read(a, "plan").expect("read a").is_none(),
        "gone from A"
    );
}

#[test]
fn updated_at_stamps_the_last_body_write_and_survives_metadata_changes() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");

    // Create stamps the create time.
    let created = written(
        store
            .write(project, "plan", &body("a"), None, 1_000)
            .expect("create"),
    );
    assert_eq!(created.updated_at, 1_000, "a create stamps updated_at");

    // A body write advances it to the write's clock.
    let updated = written(
        store
            .write(project, "plan", &body("b"), Some(1), 5_000)
            .expect("update"),
    );
    assert_eq!(
        updated.updated_at, 5_000,
        "a body write re-stamps updated_at"
    );

    // Archiving and tagging are not body edits — they leave updated_at where the last write put it.
    let archived = store
        .set_archived(project, "plan", true)
        .expect("archive")
        .expect("exists");
    assert_eq!(
        archived.updated_at, 5_000,
        "archiving is not a body edit and does not re-stamp updated_at"
    );
    let tagged = store
        .add_tags(project, "plan", &["release".into()])
        .expect("tag")
        .expect("exists");
    assert_eq!(
        tagged.updated_at, 5_000,
        "a tag change does not re-stamp updated_at"
    );
}
