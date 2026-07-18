use rusqlite::Connection;
use soloist_core::{ProjectId, ScratchpadRepo, TodoDoc, TodoId, TodoRepo, TodoStatus};
use tempfile::tempdir;

use super::*;
use crate::SqliteStore;

// A full legacy scratchpad document, exactly as an older build stored it in the `doc` column.
const LEGACY_SCRATCHPAD: &str = r#"{
    "objective": "Ship v1",
    "context": "RC cut",
    "plan": ["Cut RC", "Soak"],
    "acceptance_criteria": ["soak green", "no leaks"],
    "risks": ["glibc"],
    "status": "in progress",
    "notes": "watch CI"
}"#;

// A full legacy todo document, exactly as an older build stored it in the `doc` column.
const LEGACY_TODO: &str = r#"{
    "title": "Add the CSV export endpoint",
    "description": "Add GET /export and wire it to the report service.",
    "acceptance_criteria": ["GET /export returns 200", "streams a CSV body"],
    "risks": ["none identified"],
    "status": "in_progress"
}"#;

#[test]
fn renders_a_full_scratchpad_document_to_canonical_sections() {
    // Every field is carried into the Markdown, in the old canonical order; the name is not embedded.
    assert_eq!(
        scratchpad_body(LEGACY_SCRATCHPAD),
        "## Objective\nShip v1\n\n\
         ## Context\nRC cut\n\n\
         ## Plan\n1. Cut RC\n2. Soak\n\n\
         ## Acceptance criteria\n- [ ] soak green\n- [ ] no leaks\n\n\
         ## Risks\n- glibc\n\n\
         ## Status\nin progress\n\n\
         ## Notes\nwatch CI\n"
    );
}

#[test]
fn omits_an_empty_notes_section() {
    let no_notes = r#"{
        "objective": "Ship v1",
        "context": "RC cut",
        "plan": ["Cut RC"],
        "acceptance_criteria": ["soak green"],
        "risks": ["glibc"],
        "status": "done",
        "notes": null
    }"#;
    let body = scratchpad_body(no_notes);
    assert!(
        !body.contains("## Notes"),
        "an absent notes field yields no section"
    );
    assert!(body.ends_with("## Status\ndone\n"));
}

#[test]
fn leaves_an_already_markdown_scratchpad_untouched() {
    // A re-run (or a body written directly as Markdown) is not the legacy JSON shape, so it is kept
    // byte-for-byte — the conversion is idempotent.
    let markdown = "## Objective\nShip v1\n\n## Status\ndone\n";
    assert_eq!(scratchpad_body(markdown), markdown);
}

#[test]
fn converts_a_full_todo_document_keeping_title_and_status() {
    let converted = todo_doc(LEGACY_TODO);
    let doc: TodoDoc = serde_json::from_str(&converted).expect("valid new-shape todo doc");
    assert_eq!(doc.title, "Add the CSV export endpoint");
    assert_eq!(doc.status, TodoStatus::InProgress);
    assert_eq!(
        doc.body,
        "Add GET /export and wire it to the report service.\n\n\
         ## Acceptance criteria\n- [ ] GET /export returns 200\n- [ ] streams a CSV body\n\n\
         ## Risks\n- none identified"
    );
}

#[test]
fn leaves_an_already_converted_todo_untouched() {
    // A todo `doc` that already carries a `body` field is new-shape; the step returns it unchanged.
    let new_shape = r#"{"title":"t","body":"already markdown","status":"open"}"#;
    assert_eq!(todo_doc(new_shape), new_shape);
}

#[test]
fn migration_converts_real_seeded_rows_read_back_through_the_repo() {
    // A durable database an older build left at v12, with a real scratchpad and todo row carrying the
    // old structured JSON. Opening it runs the v13 conversion; the rows read back through the repos
    // as Markdown, with every field preserved (zero data loss).
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("soloist.db");
    seed_v12_database(&db);

    let store = SqliteStore::open(&db).expect("open runs the v13 migration");
    let project = ProjectId::from_raw(1);

    let pad = ScratchpadRepo::read(&store, project, "plan")
        .expect("read scratchpad")
        .expect("the seeded scratchpad survives");
    for fragment in [
        "## Objective",
        "Ship v1",
        "RC cut",
        "1. Cut RC",
        "2. Soak",
        "- [ ] soak green",
        "- [ ] no leaks",
        "- glibc",
        "in progress",
        "## Notes\nwatch CI",
    ] {
        assert!(
            pad.body.contains(fragment),
            "the migrated scratchpad body keeps `{fragment}`; body was:\n{}",
            pad.body
        );
    }

    let todo = TodoRepo::read(&store, project, TodoId::from_raw(1))
        .expect("read todo")
        .expect("the seeded todo survives");
    assert_eq!(todo.doc.title, "Add the CSV export endpoint");
    assert_eq!(todo.doc.status, TodoStatus::InProgress);
    for fragment in [
        "Add GET /export and wire it to the report service.",
        "## Acceptance criteria",
        "- [ ] GET /export returns 200",
        "- [ ] streams a CSV body",
        "## Risks",
        "- none identified",
    ] {
        assert!(
            todo.doc.body.contains(fragment),
            "the migrated todo body keeps `{fragment}`; body was:\n{}",
            todo.doc.body
        );
    }
}

/// Seeds a database at schema version 12 with the projects, scratchpads, and todos tables an older
/// build had, one project plus one structured scratchpad and todo row, then closes it so the store
/// can reopen and migrate it.
fn seed_v12_database(db: &std::path::Path) {
    let conn = Connection::open(db).expect("open seed db");
    conn.execute_batch(
        "CREATE TABLE projects (
             id   INTEGER PRIMARY KEY,
             root TEXT NOT NULL UNIQUE,
             name TEXT,
             icon TEXT
         );
         CREATE TABLE scratchpads (
             id         INTEGER PRIMARY KEY AUTOINCREMENT,
             project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
             name       TEXT NOT NULL,
             doc        TEXT NOT NULL,
             tags       TEXT NOT NULL,
             archived   INTEGER NOT NULL DEFAULT 0,
             revision   INTEGER NOT NULL,
             UNIQUE (project_id, name)
         );
         CREATE TABLE todos (
             id         INTEGER PRIMARY KEY AUTOINCREMENT,
             project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
             doc        TEXT NOT NULL,
             tags       TEXT NOT NULL DEFAULT '[]',
             blockers   TEXT NOT NULL DEFAULT '[]',
             comments   TEXT NOT NULL DEFAULT '[]',
             locked_by  INTEGER,
             revision   INTEGER NOT NULL
         );",
    )
    .expect("seed the v12 schema");
    conn.execute("INSERT INTO projects (id, root) VALUES (1, '/tmp/p')", [])
        .expect("seed a project");
    conn.execute(
        "INSERT INTO scratchpads (project_id, name, doc, tags, revision) VALUES (1, 'plan', ?1, '[]', 3)",
        [LEGACY_SCRATCHPAD],
    )
    .expect("seed a scratchpad");
    conn.execute(
        "INSERT INTO todos (project_id, doc, revision) VALUES (1, ?1, 2)",
        [LEGACY_TODO],
    )
    .expect("seed a todo");
    conn.pragma_update(None, "user_version", 12)
        .expect("mark it as a v12 database");
}
