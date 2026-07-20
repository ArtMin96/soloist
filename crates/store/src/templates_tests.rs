use std::sync::{Arc, Barrier};

use rusqlite::Connection;
use soloist_core::{ProjectId, StoredTemplate, TemplateKind, TemplateRepo, TemplateWriteResult};
use tempfile::tempdir;

use crate::SqliteStore;

const P: ProjectId = ProjectId::from_raw(1);
const OTHER: ProjectId = ProjectId::from_raw(2);
const PROMPT: TemplateKind = TemplateKind::Prompt;

fn store_with_project() -> SqliteStore {
    let store = SqliteStore::open_in_memory().expect("in-memory store");
    seed_project(&store, P, "/p");
    store
}

/// A store holding both projects, for the tests that must tell one project's scope from another's.
fn store_with_two_projects() -> SqliteStore {
    let store = store_with_project();
    seed_project(&store, OTHER, "/other");
    store
}

fn seed_project(store: &SqliteStore, id: ProjectId, root: &str) {
    store
        .lock()
        .execute(
            "INSERT INTO projects (id, root, name) VALUES (?1, ?2, ?3)",
            (id.get() as i64, root, root),
        )
        .expect("seed the project");
}

fn written(result: TemplateWriteResult) -> StoredTemplate {
    match result {
        TemplateWriteResult::Written(stored) => *stored,
        TemplateWriteResult::Conflict { actual } => {
            panic!("expected a write, got a conflict at {actual:?}")
        }
    }
}

#[test]
fn create_then_read_round_trips_in_both_scopes() {
    let store = store_with_project();

    let global = written(
        store
            .write(PROMPT, None, "review", Some("desc"), "global {{a}}", None)
            .expect("global create"),
    );
    let project = written(
        store
            .write(PROMPT, Some(P), "review", None, "project {{b}}", None)
            .expect("project create"),
    );

    assert_eq!(global.project, None);
    assert_eq!(global.kind, TemplateKind::Prompt);
    assert_eq!(global.revision, 1);
    assert_eq!(project.project, Some(P));
    assert_ne!(global.id, project.id);
    assert_eq!(
        store
            .read(PROMPT, None, "review")
            .expect("read")
            .expect("present"),
        global
    );
    assert_eq!(
        store
            .read(PROMPT, Some(P), "review")
            .expect("read")
            .expect("present"),
        project
    );
}

#[test]
fn a_write_is_revision_guarded() {
    let store = store_with_project();
    store
        .write(PROMPT, Some(P), "t", None, "one", None)
        .expect("create");

    let updated = written(
        store
            .write(PROMPT, Some(P), "t", Some("d"), "two", Some(1))
            .expect("update at the current revision"),
    );
    assert_eq!(updated.revision, 2);
    assert_eq!(updated.description.as_deref(), Some("d"));

    match store
        .write(PROMPT, Some(P), "t", None, "clobber", Some(1))
        .expect("a stale write resolves, not errors")
    {
        TemplateWriteResult::Conflict { actual: Some(2) } => {}
        other => panic!("expected a conflict at revision 2, got {other:?}"),
    }
    assert_eq!(
        store
            .read(PROMPT, Some(P), "t")
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
        .write(PROMPT, None, "review", None, "one", None)
        .expect("create");

    // The guarded path reports the conflict.
    match store
        .write(PROMPT, None, "review", None, "two", None)
        .expect("a duplicate create resolves, not errors")
    {
        TemplateWriteResult::Conflict { actual: Some(1) } => {}
        other => panic!("expected a name-taken conflict, got {other:?}"),
    }

    // The index is the backstop: NULLs are distinct inside a UNIQUE constraint, so only the
    // COALESCE expression index makes a raw duplicate INSERT fail.
    let raw = store.lock().execute(
        "INSERT INTO templates (kind, project_id, name, description, body, revision)
         VALUES ('prompt', NULL, 'review', NULL, 'sneaky', 1)",
        [],
    );
    assert!(
        raw.is_err(),
        "the kind-scope-name index must reject the row"
    );
}

#[test]
fn the_same_name_lives_independently_across_kinds() {
    let store = store_with_project();
    store
        .write(PROMPT, None, "design", None, "prompt body", None)
        .expect("prompt create");
    // The same name under another kind is a distinct row — the unique index keys on kind too.
    let scratch = written(
        store
            .write(
                TemplateKind::Scratchpad,
                None,
                "design",
                None,
                "# scratchpad shape",
                None,
            )
            .expect("scratchpad create"),
    );
    assert_eq!(scratch.kind, TemplateKind::Scratchpad);
    assert_ne!(
        scratch.id,
        store
            .read(PROMPT, None, "design")
            .expect("read")
            .expect("present")
            .id
    );
    // A kind-scoped list never spans another kind.
    assert_eq!(store.list(PROMPT, None).expect("prompt list").len(), 1);
    assert_eq!(
        store
            .list(TemplateKind::Scratchpad, None)
            .expect("scratchpad list")
            .len(),
        1
    );
}

#[test]
fn delete_is_scope_exact_and_reports_presence() {
    let store = store_with_two_projects();
    for scope in [None, Some(P), Some(OTHER)] {
        store
            .write(PROMPT, scope, "review", None, "body", None)
            .expect("create");
    }

    assert!(store.delete(PROMPT, None, "review").expect("delete global"));
    assert!(!store
        .delete(PROMPT, None, "review")
        .expect("re-delete is absent"));
    assert!(store
        .delete(PROMPT, Some(P), "review")
        .expect("delete in P"));
    assert!(
        store
            .read(PROMPT, Some(OTHER), "review")
            .expect("read")
            .is_some(),
        "another project's same-named row is untouched by both deletes"
    );
}

#[test]
fn list_is_scoped_and_ordered_by_name() {
    let store = store_with_project();
    store
        .write(PROMPT, Some(P), "b", None, "2", None)
        .expect("create");
    store
        .write(PROMPT, Some(P), "a", None, "1", None)
        .expect("create");
    store
        .write(PROMPT, None, "g", None, "3", None)
        .expect("create");

    let names: Vec<String> = store
        .list(PROMPT, Some(P))
        .expect("list")
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(store.list(PROMPT, None).expect("global list").len(), 1);
}

#[test]
fn count_is_scoped_like_a_list_and_follows_deletes() {
    let store = store_with_two_projects();
    for (kind, project, name) in [
        (PROMPT, Some(P), "a"),
        (PROMPT, Some(P), "b"),
        (PROMPT, None, "g"),
        (PROMPT, Some(OTHER), "theirs"),
        (TemplateKind::Scratchpad, Some(P), "shape"),
    ] {
        store
            .write(kind, project, name, None, "body", None)
            .expect("create");
    }

    // Each `(kind, scope)` counts only its own rows — the global scope included, which the NULL
    // project id makes the easy one to match by accident.
    for (kind, project, expected) in [
        (PROMPT, Some(P), 2),
        (PROMPT, None, 1),
        (PROMPT, Some(OTHER), 1),
        (TemplateKind::Scratchpad, Some(P), 1),
        (TemplateKind::Todo, Some(P), 0),
    ] {
        assert_eq!(
            store.count(kind, project).expect("count"),
            expected,
            "count of {kind:?} in {project:?}"
        );
        assert_eq!(
            store.count(kind, project).expect("count"),
            store.list(kind, project).expect("list").len(),
            "count agrees with the list it stands in for"
        );
    }

    assert!(store.delete(PROMPT, Some(P), "a").expect("delete"));
    assert_eq!(store.count(PROMPT, Some(P)).expect("count"), 1);
}

#[test]
fn templates_survive_a_store_reopen() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("soloist.db");

    let created = {
        let store = SqliteStore::open(&path).expect("open store");
        written(
            store
                .write(PROMPT, None, "keep", Some("d"), "body {{x}}", None)
                .expect("create"),
        )
    };

    let reopened = SqliteStore::open(&path).expect("reopen store");
    assert_eq!(
        reopened
            .read(PROMPT, None, "keep")
            .expect("read")
            .expect("present"),
        created
    );
}

#[test]
fn deleting_a_project_cascades_to_its_templates_and_leaves_globals() {
    let store = store_with_two_projects();
    store
        .write(PROMPT, Some(P), "mine", None, "project", None)
        .expect("create");
    store
        .write(PROMPT, Some(OTHER), "mine", None, "other project", None)
        .expect("create");
    store
        .write(PROMPT, None, "shared", None, "global", None)
        .expect("create");

    store
        .lock()
        .execute("DELETE FROM projects WHERE id = ?1", [P.get() as i64])
        .expect("drop the project");

    assert!(store.read(PROMPT, Some(P), "mine").expect("read").is_none());
    assert!(store.read(PROMPT, None, "shared").expect("read").is_some());
    assert!(
        store
            .read(PROMPT, Some(OTHER), "mine")
            .expect("read")
            .is_some(),
        "the cascade stops at the removed project"
    );
}

#[test]
fn v14_backfills_existing_prompt_rows_as_kind_prompt() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("soloist.db");

    // Build a pre-v14 database: the v12 `prompt_templates` schema with a real seeded row, marked as
    // a v13 database (exactly what an older build left before this migration existed).
    {
        let conn = Connection::open(&path).expect("open raw");
        conn.execute_batch(
            "CREATE TABLE projects (
                 id   INTEGER PRIMARY KEY,
                 root TEXT NOT NULL UNIQUE,
                 name TEXT,
                 icon TEXT
             );
             CREATE TABLE prompt_templates (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id  INTEGER REFERENCES projects(id) ON DELETE CASCADE,
                 name        TEXT NOT NULL,
                 description TEXT,
                 body        TEXT NOT NULL,
                 revision    INTEGER NOT NULL
             );
             CREATE UNIQUE INDEX prompt_templates_scope_name
                 ON prompt_templates (COALESCE(project_id, 0), name);",
        )
        .expect("seed the pre-v14 schema");
        conn.execute("INSERT INTO projects (id, root) VALUES (1, '/tmp/p')", [])
            .expect("seed a project");
        conn.execute(
            "INSERT INTO prompt_templates (project_id, name, description, body, revision)
             VALUES (NULL, 'review', 'desc', 'Review {{diff}}', 3)",
            [],
        )
        .expect("seed a prompt-template row");
        conn.pragma_update(None, "user_version", 13)
            .expect("mark it a v13 database");
    }

    // Opening runs the migration to v14; the existing row is read back through the repo as a prompt
    // with its content, revision, and derived placeholders intact — zero data loss.
    let store = SqliteStore::open(&path).expect("open store");
    let row = store
        .read(PROMPT, None, "review")
        .expect("read")
        .expect("the backfilled row is present");
    assert_eq!(row.kind, TemplateKind::Prompt);
    assert_eq!(row.name, "review");
    assert_eq!(row.description.as_deref(), Some("desc"));
    assert_eq!(row.body, "Review {{diff}}");
    assert_eq!(row.revision, 3);

    // The new (kind, scope, name) index is in force: the same name may now exist under another
    // kind, and a duplicate prompt is still rejected at the index.
    store
        .write(TemplateKind::Todo, None, "review", None, "todo shape", None)
        .expect("the same name under another kind is allowed");
    let dup = store.lock().execute(
        "INSERT INTO templates (kind, project_id, name, description, body, revision)
         VALUES ('prompt', NULL, 'review', NULL, 'dup', 1)",
        [],
    );
    assert!(dup.is_err(), "a duplicate prompt is rejected by the index");
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
        .write(PROMPT, Some(P), "contended", None, "base", None)
        .expect("create at revision 1");
    const CONTENDERS: usize = 16;

    let barrier = Arc::new(Barrier::new(CONTENDERS));
    let outcomes: Vec<TemplateWriteResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..CONTENDERS)
            .map(|n| {
                let store = store.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    store
                        .write(
                            PROMPT,
                            Some(P),
                            "contended",
                            None,
                            &format!("edit-{n}"),
                            Some(1),
                        )
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
        .filter(|outcome| matches!(outcome, TemplateWriteResult::Written(_)))
        .count();
    let conflicts = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, TemplateWriteResult::Conflict { actual: Some(2) }))
        .count();
    assert_eq!(applied, 1, "exactly one write at revision 1 applies");
    assert_eq!(
        conflicts,
        CONTENDERS - 1,
        "every other writer is refused against the single bumped revision"
    );
    assert_eq!(
        store
            .read(PROMPT, Some(P), "contended")
            .expect("read")
            .expect("present")
            .revision,
        2
    );
}

#[test]
fn one_projects_scope_never_matches_another_projects_rows() {
    let store = store_with_two_projects();
    // The same name in three scopes: the unique index keys on (kind, scope, name), so all three
    // coexist and only the scope tells them apart.
    for (scope, body) in [(None, "global"), (Some(P), "mine"), (Some(OTHER), "theirs")] {
        written(
            store
                .write(PROMPT, scope, "review", None, body, None)
                .expect("create in the scope"),
        );
    }

    // A read in one project's scope resolves that project's row — the value-vs-value branch of the
    // scope filter, which a single-project fixture can never exercise.
    for (scope, expected) in [(None, "global"), (Some(P), "mine"), (Some(OTHER), "theirs")] {
        let read = store
            .read(PROMPT, scope, "review")
            .expect("read")
            .expect("the scope has its own row");
        assert_eq!(read.body, expected, "scope {scope:?} read the wrong row");
        assert_eq!(read.project, scope);
        let listed = store.list(PROMPT, scope).expect("list");
        assert_eq!(listed.len(), 1, "scope {scope:?} lists only its own row");
        assert_eq!(listed[0].body, expected);
    }
}

#[test]
fn a_row_of_a_kind_this_build_does_not_know_is_invisible_rather_than_fatal() {
    let store = store_with_project();
    store
        .write(PROMPT, Some(P), "review", None, "body", None)
        .expect("seed a readable row");
    // A row a newer build wrote carries a `kind` this one has no variant for. Every query filters on
    // a known kind, so such a row is simply not addressed — an older build keeps working against a
    // table a newer one has written to, instead of failing every read of the scope.
    store
        .lock()
        .execute(
            "INSERT INTO templates (kind, project_id, name, description, body, revision)
             VALUES ('grimoire', ?1, 'future', NULL, 'body', 1)",
            (P.get() as i64,),
        )
        .expect("insert an unrecognised kind");

    assert!(store
        .read(PROMPT, Some(P), "future")
        .expect("a read of the scope still succeeds")
        .is_none());
    assert_eq!(
        store
            .list(PROMPT, Some(P))
            .expect("a list of the scope still succeeds")
            .into_iter()
            .map(|row| row.name)
            .collect::<Vec<_>>(),
        vec!["review".to_owned()]
    );
    assert_eq!(store.count(PROMPT, Some(P)).expect("count"), 1);
}
