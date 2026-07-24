use super::*;

#[test]
fn migrates_a_fresh_database_to_the_current_schema() {
    let conn = Connection::open_in_memory().expect("in-memory db");

    migrate(&conn).expect("a fresh database migrates cleanly");

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read user_version");
    assert_eq!(
        version, SCHEMA_VERSION,
        "migration must advance a fresh database to the current schema version"
    );

    for table in [
        "meta",
        "projects",
        "trust",
        "agent_tools",
        "leases",
        "timers",
        "scratchpads",
        "todos",
        "kv",
        "settings",
        "project_settings",
        "feedback",
        "templates",
        "diagrams",
    ] {
        let exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |_| Ok(()),
            )
            .is_ok();
        assert!(exists, "migration must create the `{table}` table");
    }

    // The built-in agent providers are seeded on the fresh database.
    let seeded: i64 = conn
        .query_row("SELECT COUNT(*) FROM agent_tools", [], |row| row.get(0))
        .expect("count seeded agent tools");
    assert_eq!(
        seeded,
        AgentTool::builtin_defaults().len() as i64,
        "migration must seed the built-in agent providers"
    );

    // Re-running over an already-current database touches nothing (idempotent steps).
    migrate(&conn).expect("re-running migrate on a current database is a no-op");
}

#[test]
fn upgrading_a_populated_intermediate_database_preserves_its_rows() {
    // A v6 database (scratchpads landed at 6) with real rows, exactly as an older build left it.
    let conn = Connection::open_in_memory().expect("in-memory db");
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
         );",
    )
    .expect("seed the v6 schema");
    conn.execute("INSERT INTO projects (id, root) VALUES (1, '/tmp/p')", [])
        .expect("seed a project row");
    conn.execute(
        "INSERT INTO scratchpads (project_id, name, doc, tags, revision) \
         VALUES (1, 'note', '{}', '[]', 1)",
        [],
    )
    .expect("seed a scratchpad row");
    conn.pragma_update(None, "user_version", 6)
        .expect("mark it as a v6 database");

    migrate(&conn).expect("a populated intermediate database upgrades");

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read user_version");
    assert_eq!(
        version, SCHEMA_VERSION,
        "the upgrade advances a populated database to the current schema"
    );

    // A table added after v6 now exists — the unified `templates` table (the prompt-templates
    // table generalized by v14), and the pre-v14 `prompt_templates` name is gone.
    assert!(
        table_exists(&conn, "templates").expect("check templates"),
        "the upgrade creates tables added after the intermediate version"
    );
    assert!(
        !table_exists(&conn, "prompt_templates").expect("check prompt_templates"),
        "v14 renames prompt_templates away"
    );

    // ...and the pre-existing rows survive it.
    let scratchpads: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM scratchpads WHERE name = 'note'",
            [],
            |row| row.get(0),
        )
        .expect("count the seeded scratchpad");
    assert_eq!(
        scratchpads, 1,
        "rows in an intermediate-version database survive the upgrade"
    );

    // v15 adds `updated_at`; a pre-existing row backfills to 0 (its last-write time is unknown)
    // rather than failing the NOT NULL column or being dropped.
    let updated_at: i64 = conn
        .query_row(
            "SELECT updated_at FROM scratchpads WHERE name = 'note'",
            [],
            |row| row.get(0),
        )
        .expect("read the backfilled updated_at");
    assert_eq!(
        updated_at, 0,
        "an intermediate-version scratchpad backfills updated_at to 0"
    );
}

#[test]
fn a_pre_v16_todo_upgrades_to_an_unlinked_one_readable_through_the_repo() {
    use soloist_core::{TodoRepo, TodoStatus};

    // A v15 database with a real todo row, exactly as the previous build left it: the `todos`
    // table has no `scratchpad_id` column at all.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("pre-v16.db");
    let conn = Connection::open(&path).expect("open");
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
             updated_at INTEGER NOT NULL DEFAULT 0,
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
    .expect("seed the v15 schema");
    conn.execute("INSERT INTO projects (id, root) VALUES (1, '/tmp/p')", [])
        .expect("seed a project row");
    conn.execute(
        "INSERT INTO todos (project_id, doc, tags, revision) \
         VALUES (1, '{\"title\":\"ship\",\"body\":\"do it\",\"status\":\"open\"}', \
                 '[\"release\"]', 3)",
        [],
    )
    .expect("seed a todo row");
    conn.pragma_update(None, "user_version", 15)
        .expect("mark it as a v15 database");
    drop(conn);

    // Opening through the store runs the migration; the row then reads back through the repo.
    let store = crate::SqliteStore::open(&path).expect("upgrade and open");
    let todo = TodoRepo::list(&store, soloist_core::ProjectId::from_raw(1))
        .expect("list through the repo")
        .pop()
        .expect("the seeded todo survives the upgrade");

    assert_eq!(todo.doc.title, "ship");
    assert_eq!(todo.doc.body, "do it");
    assert_eq!(todo.doc.status, TodoStatus::Open);
    assert_eq!(todo.tags, vec!["release".to_owned()]);
    assert_eq!(todo.revision, 3);
    assert_eq!(
        todo.scratchpad, None,
        "an existing todo is left unlinked — an association is stated, never inferred"
    );
}

#[test]
fn refuses_a_schema_newer_than_this_build() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
        .expect("set a future schema version");
    assert!(
        migrate(&conn).is_err(),
        "a newer schema must be refused, not silently downgraded"
    );
}
