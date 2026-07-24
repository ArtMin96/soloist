use std::path::Path;
use std::sync::{Arc, Barrier};

use soloist_core::{
    DiagramId, DiagramRenameResult, DiagramRepo, DiagramWriteResult, ProjectId, ProjectRepo,
    StoreError, StoredDiagram,
};
use tempfile::tempdir;

use super::*;

/// A fixed wall clock for the writes whose recency is not under test — these exercise revision,
/// rename, tag, and archive semantics, and `updated_at` is verified on its own below.
const FIXED_NOW: u64 = 1_700_000_000_000;

/// Writes at [`FIXED_NOW`], so the semantics tests read the same regardless of the recency clock. The
/// recency-stamping behaviour has its own test that drives real timestamps.
trait WriteAt {
    fn write_at(
        &self,
        project: ProjectId,
        name: &str,
        source: &str,
        expected: Option<u64>,
    ) -> Result<DiagramWriteResult, StoreError>;
}

impl WriteAt for SqliteStore {
    fn write_at(
        &self,
        project: ProjectId,
        name: &str,
        source: &str,
        expected: Option<u64>,
    ) -> Result<DiagramWriteResult, StoreError> {
        DiagramRepo::write(self, project, name, source, expected, FIXED_NOW)
    }
}

fn project(store: &SqliteStore, root: &str) -> ProjectId {
    store
        .upsert(Path::new(root), None, None)
        .expect("project for diagram fk")
        .id
}

/// A representative Mermaid source carrying `marker` so writes can be told apart.
fn source(marker: &str) -> String {
    format!("flowchart TD\n  A --> B\n  %% {marker}")
}

fn written(result: DiagramWriteResult) -> StoredDiagram {
    match result {
        DiagramWriteResult::Written(stored) => *stored,
        DiagramWriteResult::Conflict { actual } => {
            panic!("expected a write, got a conflict at {actual:?}")
        }
    }
}

#[test]
fn create_then_read_round_trips_the_source() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");

    let created = written(
        store
            .write_at(project, "flow", &source("started"), None)
            .expect("create"),
    );
    assert_eq!(created.revision, 1);
    assert!(created.id.get() > 0, "the store assigns a durable id");

    let read = DiagramRepo::read(&store, project, "flow")
        .expect("read")
        .expect("exists");
    // The Mermaid source survives the store round-trip verbatim.
    assert_eq!(read.source, source("started"));
    assert_eq!(read, created);
}

#[test]
fn contains_answers_membership_per_project_not_per_id() {
    let store = SqliteStore::open_in_memory().expect("open");
    let mine = project(&store, "/p/mine");
    let theirs = project(&store, "/p/theirs");
    let diagram = written(
        store
            .write_at(theirs, "their-flow", &source("theirs"), None)
            .expect("create in the other project"),
    );

    assert!(store.contains(theirs, diagram.id).expect("own project"));
    assert!(
        !store.contains(mine, diagram.id).expect("other project"),
        "a real row must not count as a member of a project that does not own it"
    );
    assert!(
        !store
            .contains(mine, DiagramId::from_raw(diagram.id.get() + 1))
            .expect("unknown id"),
        "an id no row carries is not a member of anything"
    );
}

#[test]
fn a_write_is_revision_guarded() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    store
        .write_at(project, "flow", &source("a"), None)
        .expect("create");

    // Update at the current revision bumps it.
    let updated = written(
        store
            .write_at(project, "flow", &source("b"), Some(1))
            .expect("update"),
    );
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.source, source("b"));

    // A stale revision conflicts and changes nothing.
    assert_eq!(
        store
            .write_at(project, "flow", &source("c"), Some(1))
            .expect("stale"),
        DiagramWriteResult::Conflict { actual: Some(2) }
    );
    assert_eq!(
        DiagramRepo::read(&store, project, "flow")
            .unwrap()
            .unwrap()
            .source,
        source("b")
    );

    // Creating over an existing name conflicts.
    assert_eq!(
        store
            .write_at(project, "flow", &source("d"), None)
            .expect("recreate"),
        DiagramWriteResult::Conflict { actual: Some(2) }
    );

    // Updating a missing diagram conflicts with no record.
    assert_eq!(
        store
            .write_at(project, "absent", &source("e"), Some(5))
            .expect("update missing"),
        DiagramWriteResult::Conflict { actual: None }
    );
}

#[test]
fn rename_keeps_the_id_and_enforces_uniqueness() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    let created = written(
        store
            .write_at(project, "old", &source("a"), None)
            .expect("create"),
    );
    store
        .write_at(project, "taken", &source("a"), None)
        .expect("create taken");

    let renamed = match store.rename(project, "old", "new").expect("rename") {
        DiagramRenameResult::Renamed(stored) => *stored,
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
        DiagramRenameResult::NotFound
    );
    assert_eq!(
        store
            .rename(project, "new", "taken")
            .expect("rename onto taken"),
        DiagramRenameResult::NameTaken
    );
}

#[test]
fn tags_add_dedupe_remove_and_list_distinct() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    store
        .write_at(project, "a", &source("a"), None)
        .expect("create a");
    store
        .write_at(project, "b", &source("a"), None)
        .expect("create b");

    let tagged = store
        .add_tags(project, "a", &["arch".into(), "arch".into(), "v1".into()])
        .expect("add")
        .expect("exists");
    assert_eq!(tagged.tags, vec!["arch".to_string(), "v1".to_string()]);

    store.add_tags(project, "b", &["v1".into()]).expect("tag b");
    assert_eq!(
        DiagramRepo::tags(&store, project).expect("tags"),
        vec!["arch".to_string(), "v1".to_string()]
    );

    let untagged = store
        .remove_tags(project, "a", &["arch".into()])
        .expect("remove")
        .expect("exists");
    assert_eq!(untagged.tags, vec!["v1".to_string()]);

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
        .write_at(project, "a", &source("a"), None)
        .expect("create");

    let archived = store
        .set_archived(project, "a", true)
        .expect("archive")
        .expect("exists");
    assert!(archived.archived);
    assert!(
        DiagramRepo::read(&store, project, "a").unwrap().is_some(),
        "archive keeps the document"
    );

    assert!(store.delete(project, "a").expect("delete"));
    assert!(!store.delete(project, "a").expect("second delete"));
    assert!(DiagramRepo::read(&store, project, "a").unwrap().is_none());
}

#[test]
fn list_is_scoped_and_ordered_by_name() {
    let store = SqliteStore::open_in_memory().expect("open");
    let one = project(&store, "/p/one");
    let two = project(&store, "/p/two");
    store
        .write_at(one, "zebra", &source("a"), None)
        .expect("create");
    store
        .write_at(one, "alpha", &source("a"), None)
        .expect("create");
    store
        .write_at(two, "other", &source("a"), None)
        .expect("create");

    let names: Vec<String> = DiagramRepo::list(&store, one)
        .expect("list")
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, vec!["alpha".to_string(), "zebra".to_string()]);
}

#[test]
fn diagrams_survive_a_store_reopen() {
    // Coordination content persists across an app restart: like scratchpads, diagrams are durable
    // and not cleared on launch.
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    let id = {
        let store = SqliteStore::open(&db).expect("open");
        let project = project(&store, "/p/app");
        let created = written(
            store
                .write_at(project, "flow", &source("started"), None)
                .expect("create"),
        );
        store
            .add_tags(project, "flow", &["arch".into()])
            .expect("tag");
        created.id
    };

    // A fresh process opens the same database: the diagram is still there, with its id, source, and
    // tags intact.
    let store = SqliteStore::open(&db).expect("reopen");
    let reopened = DiagramRepo::read(&store, ProjectId::from_raw(1), "flow")
        .expect("read")
        .expect("the diagram survives the reopen");
    assert_eq!(reopened.id, id);
    assert_eq!(reopened.source, source("started"));
    assert_eq!(reopened.tags, vec!["arch".to_string()]);
}

#[test]
fn deleting_a_project_cascades_to_its_diagrams() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");
    store
        .write_at(project, "flow", &source("a"), None)
        .expect("create");

    store.remove(project).expect("remove project");
    assert!(
        DiagramRepo::read(&store, project, "flow")
            .expect("read")
            .is_none(),
        "the project's diagrams are dropped with it"
    );
}

#[test]
fn concurrent_writes_at_one_revision_apply_exactly_one() {
    // The race the atomic revision guard fixes: many agents update one diagram from the same revision
    // at once. Exactly one write must apply (bumping the revision); every other must be refused as a
    // conflict — never two writes accepted at one revision.
    let dir = tempdir().expect("temp dir");
    let store = Arc::new(SqliteStore::open(&dir.path().join("soloist.db")).expect("open"));
    let project = project(&store, "/p/race");
    store
        .write_at(project, "flow", &source("base"), None)
        .expect("create at revision 1");
    const CONTENDERS: u64 = 16;

    let barrier = Arc::new(Barrier::new(CONTENDERS as usize));
    let outcomes: Vec<DiagramWriteResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CONTENDERS)
            .map(|n| {
                let store = store.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    store
                        .write_at(project, "flow", &source(&format!("edit-{n}")), Some(1))
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
        .filter(|outcome| matches!(outcome, DiagramWriteResult::Written(_)))
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, DiagramWriteResult::Conflict { actual: Some(2) }))
        .count();
    assert_eq!(applied, 1, "exactly one write at revision 1 applies");
    assert_eq!(
        conflicts,
        (CONTENDERS - 1) as usize,
        "every other writer is refused against the single bumped revision"
    );
    assert_eq!(store.read(project, "flow").unwrap().unwrap().revision, 2);
}

#[test]
fn updated_at_stamps_the_last_source_write_and_survives_metadata_changes() {
    let store = SqliteStore::open_in_memory().expect("open");
    let project = project(&store, "/p/app");

    // Create stamps the create time.
    let created = written(
        DiagramRepo::write(&store, project, "flow", &source("a"), None, 1_000).expect("create"),
    );
    assert_eq!(created.updated_at, 1_000, "a create stamps updated_at");

    // A source write advances it to the write's clock.
    let updated = written(
        DiagramRepo::write(&store, project, "flow", &source("b"), Some(1), 5_000).expect("update"),
    );
    assert_eq!(
        updated.updated_at, 5_000,
        "a source write re-stamps updated_at"
    );

    // Archiving and tagging are not source edits — they leave updated_at where the last write put it.
    let archived = store
        .set_archived(project, "flow", true)
        .expect("archive")
        .expect("exists");
    assert_eq!(
        archived.updated_at, 5_000,
        "archiving is not a source edit and does not re-stamp updated_at"
    );
    let tagged = store
        .add_tags(project, "flow", &["arch".into()])
        .expect("tag")
        .expect("exists");
    assert_eq!(
        tagged.updated_at, 5_000,
        "a tag change does not re-stamp updated_at"
    );
}
