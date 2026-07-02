use std::sync::{Arc, Barrier};

use soloist_core::{ProjectId, PromptTemplateRepo, PromptTemplateWriteResult};
use tempfile::tempdir;

use crate::SqliteStore;

const P: ProjectId = ProjectId::from_raw(1);

fn store_with_project() -> SqliteStore {
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    store
        .lock()
        .execute(
            "INSERT INTO projects (id, root, name) VALUES (?1, ?2, ?3)",
            (P.get() as i64, "/p", "P"),
        )
        .expect("seed project P");
    store
}

fn written(result: PromptTemplateWriteResult) -> soloist_core::StoredPromptTemplate {
    match result {
        PromptTemplateWriteResult::Written(stored) => *stored,
        PromptTemplateWriteResult::Conflict { actual } => {
            panic!("expected a write, got a conflict at {actual:?}")
        }
    }
}

#[test]
fn create_then_read_round_trips_in_both_scopes() {
    let store = store_with_project();

    let global = written(
        store
            .write(None, "review", Some("desc"), "global {{a}}", None)
            .expect("global create"),
    );
    let project = written(
        store
            .write(Some(P), "review", None, "project {{b}}", None)
            .expect("project create"),
    );

    assert_eq!(global.project, None);
    assert_eq!(global.revision, 1);
    assert_eq!(project.project, Some(P));
    assert_ne!(global.id, project.id);
    assert_eq!(
        store.read(None, "review").expect("read").expect("present"),
        global
    );
    assert_eq!(
        store
            .read(Some(P), "review")
            .expect("read")
            .expect("present"),
        project
    );
}

#[test]
fn a_write_is_revision_guarded() {
    let store = store_with_project();
    store
        .write(Some(P), "t", None, "one", None)
        .expect("create");

    let updated = written(
        store
            .write(Some(P), "t", Some("d"), "two", Some(1))
            .expect("update at the current revision"),
    );
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.description.as_deref(), Some("d"));

    match store
        .write(Some(P), "t", None, "clobber", Some(1))
        .expect("a stale write resolves, not errors")
    {
        PromptTemplateWriteResult::Conflict { actual: Some(2) } => {}
        other => panic!("expected a conflict at revision 2, got {other:?}"),
    }
    assert_eq!(
        store
            .read(Some(P), "t")
            .expect("read")
            .expect("present")
            .body,
        "two"
    );
}

#[test]
fn two_same_named_globals_are_rejected_even_bypassing_the_guarded_path() {
    let store = store_with_project();
    store
        .write(None, "review", None, "one", None)
        .expect("create");

    // The guarded path reports the conflict.
    match store
        .write(None, "review", None, "two", None)
        .expect("a duplicate create resolves, not errors")
    {
        PromptTemplateWriteResult::Conflict { actual: Some(1) } => {}
        other => panic!("expected a name-taken conflict, got {other:?}"),
    }

    // The index is the backstop: NULLs are distinct inside a UNIQUE constraint, so only the
    // COALESCE expression index makes a raw duplicate INSERT fail.
    let raw = store.lock().execute(
        "INSERT INTO prompt_templates (project_id, name, description, body, revision)
         VALUES (NULL, 'review', NULL, 'sneaky', 1)",
        [],
    );
    assert!(raw.is_err(), "the scope-name index must reject the row");
}

#[test]
fn delete_is_scope_exact_and_reports_presence() {
    let store = store_with_project();
    store
        .write(None, "review", None, "global", None)
        .expect("create");
    store
        .write(Some(P), "review", None, "project", None)
        .expect("create");

    assert!(store.delete(None, "review").expect("delete global"));
    assert!(!store.delete(None, "review").expect("re-delete is absent"));
    assert!(
        store.read(Some(P), "review").expect("read").is_some(),
        "the project row is untouched"
    );
}

#[test]
fn list_is_scoped_and_ordered_by_name() {
    let store = store_with_project();
    store.write(Some(P), "b", None, "2", None).expect("create");
    store.write(Some(P), "a", None, "1", None).expect("create");
    store.write(None, "g", None, "3", None).expect("create");

    let names: Vec<String> = store
        .list(Some(P))
        .expect("list")
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(store.list(None).expect("global list").len(), 1);
}

#[test]
fn templates_survive_a_store_reopen() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("soloist.db");

    let created = {
        let store = SqliteStore::open(&path).expect("open store");
        written(
            store
                .write(None, "keep", Some("d"), "body {{x}}", None)
                .expect("create"),
        )
    };

    let reopened = SqliteStore::open(&path).expect("reopen store");
    assert_eq!(
        reopened.read(None, "keep").expect("read").expect("present"),
        created
    );
}

#[test]
fn deleting_a_project_cascades_to_its_templates_and_leaves_globals() {
    let store = store_with_project();
    store
        .write(Some(P), "mine", None, "project", None)
        .expect("create");
    store
        .write(None, "shared", None, "global", None)
        .expect("create");

    store
        .lock()
        .execute("DELETE FROM projects WHERE id = ?1", [P.get() as i64])
        .expect("drop the project");

    assert!(store.read(Some(P), "mine").expect("read").is_none());
    assert!(store.read(None, "shared").expect("read").is_some());
}

#[test]
fn concurrent_writes_at_one_revision_apply_exactly_one() {
    // The race the atomic revision guard fixes: many agents update one template from the same
    // revision at once. Exactly one write must apply; every other is refused as a conflict.
    let dir = tempdir().expect("temp dir");
    let store = Arc::new(SqliteStore::open(&dir.path().join("soloist.db")).expect("open"));
    store
        .lock()
        .execute(
            "INSERT INTO projects (id, root, name) VALUES (?1, ?2, ?3)",
            (P.get() as i64, "/p/race", "P"),
        )
        .expect("seed project P");
    store
        .write(Some(P), "contended", None, "base", None)
        .expect("create at revision 1");
    const CONTENDERS: usize = 16;

    let barrier = Arc::new(Barrier::new(CONTENDERS));
    let outcomes: Vec<PromptTemplateWriteResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CONTENDERS)
            .map(|n| {
                let store = store.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    store
                        .write(Some(P), "contended", None, &format!("edit-{n}"), Some(1))
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
        .filter(|outcome| matches!(outcome, PromptTemplateWriteResult::Written(_)))
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome,
                PromptTemplateWriteResult::Conflict { actual: Some(2) }
            )
        })
        .count();
    assert_eq!(applied, 1, "exactly one write at revision 1 applies");
    assert_eq!(
        conflicts,
        CONTENDERS - 1,
        "every other writer is refused against the single bumped revision"
    );
    assert_eq!(
        store
            .read(Some(P), "contended")
            .expect("read")
            .expect("present")
            .revision,
        2
    );
}
