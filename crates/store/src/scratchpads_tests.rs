use std::path::Path;
use std::sync::{Arc, Barrier};

use soloist_core::{
    ProjectId, ProjectRepo, RenameResult, ScratchpadRepo, StoredScratchpad, TransferResult,
    WriteResult,
};
use tempfile::tempdir;

use super::*;

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

#[test]
fn create_then_read_round_trips_the_body() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");

    let created = written(
        store
            .write(project, "plan", &body("started"), None)
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
        .write(project, "plan", &body("a"), None)
        .expect("create");

    // Update at the current revision bumps it.
    let updated = written(
        store
            .write(project, "plan", &body("b"), Some(1))
            .expect("update"),
    );
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.body, body("b"));

    // A stale revision conflicts and changes nothing.
    assert_eq!(
        store
            .write(project, "plan", &body("c"), Some(1))
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
            .write(project, "plan", &body("d"), None)
            .expect("recreate"),
        WriteResult::Conflict { actual: Some(2) }
    );

    // Updating a missing scratchpad conflicts with no record.
    assert_eq!(
        store
            .write(project, "absent", &body("e"), Some(5))
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
            .write(project, "old", &body("a"), None)
            .expect("create"),
    );
    store
        .write(project, "taken", &body("a"), None)
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
        .write(project, "a", &body("a"), None)
        .expect("create a");
    store
        .write(project, "b", &body("a"), None)
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
    store.write(project, "a", &body("a"), None).expect("create");

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
    store.write(one, "zebra", &body("a"), None).expect("create");
    store.write(one, "alpha", &body("a"), None).expect("create");
    store.write(two, "other", &body("a"), None).expect("create");

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
                .write(project, "plan", &body("started"), None)
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
        .write(project, "plan", &body("a"), None)
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
        .write(project, "plan", &body("base"), None)
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
                        .write(project, "plan", &body(&format!("edit-{n}")), Some(1))
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

#[test]
fn transfer_moves_the_scratchpad_keeping_identity_and_refuses_a_taken_name() {
    let store = SqliteStore::open_in_memory().expect("open");
    let a = project(&store, "/p/a");
    let b = project(&store, "/p/b");
    let created = match store
        .write(a, "plan", &body("draft"), None)
        .expect("create")
    {
        WriteResult::Written(stored) => *stored,
        other => panic!("expected a write, got {other:?}"),
    };

    // A name already used in the target is refused.
    store
        .write(b, "plan", &body("draft"), None)
        .expect("create in B");
    assert!(matches!(
        store.transfer(a, "plan", b).expect("transfer"),
        TransferResult::NameTaken
    ));

    // Clear the collision, then the move keeps the durable id and revision.
    store.delete(b, "plan").expect("delete B copy");
    let moved = match store.transfer(a, "plan", b).expect("transfer") {
        TransferResult::Transferred(stored) => *stored,
        other => panic!("expected a transfer, got {other:?}"),
    };
    assert_eq!(moved.id, created.id, "durable id kept");
    assert_eq!(moved.project, b, "now under the target project");
    assert_eq!(moved.revision, created.revision, "revision kept");
    assert!(
        store.read(a, "plan").expect("read a").is_none(),
        "gone from A"
    );
}
