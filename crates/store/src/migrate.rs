//! Versioned, idempotent SQLite migrations for the durable store.

use rusqlite::{Connection, OptionalExtension};
use soloist_core::{AgentTool, StoreError};

use crate::sql_err;

/// The newest schema version this build knows how to migrate to.
pub(crate) const SCHEMA_VERSION: i64 = 17;

/// Applies migrations newer than the database's recorded `user_version`. Each step
/// is idempotent; the version is bumped only after all pending steps succeed. A
/// database whose version is newer than this build understands is refused rather
/// than silently downgraded.
pub(crate) fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sql_err)?;

    if version > SCHEMA_VERSION {
        return Err(StoreError::Backend(format!(
            "database schema version {version} is newer than this build supports \
             ({SCHEMA_VERSION}); upgrade Soloist"
        )));
    }

    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 2 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                 id   INTEGER PRIMARY KEY,
                 root TEXT NOT NULL UNIQUE,
                 name TEXT,
                 icon TEXT
             );
             CREATE TABLE IF NOT EXISTS trust (
                 project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 variant_hash TEXT NOT NULL,
                 PRIMARY KEY (project_id, variant_hash)
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 3 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_tools (
                 name       TEXT PRIMARY KEY,
                 position   INTEGER NOT NULL,
                 definition TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
        seed_builtin_agent_tools(conn)?;
    }

    if version < 4 {
        // Coordination leases: one row per (project, key). `owner` is a per-run process id and
        // the millis are a persistable wall clock. The project foreign key cascades, so removing a
        // project drops its leases; launch reconciliation clears whatever a previous run left.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS leases (
                 project_id           INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 key                  TEXT NOT NULL,
                 owner                INTEGER NOT NULL,
                 acquired_unix_millis INTEGER NOT NULL,
                 expires_unix_millis  INTEGER NOT NULL,
                 PRIMARY KEY (project_id, key)
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 5 {
        // Coordination timers: one row per timer, with a store-assigned id that is never reused
        // (`AUTOINCREMENT`). `fire` is the JSON of the domain `FireCond` (kind + watched set), so
        // the persisted shape cannot drift; `deadline_unix_millis` is its own column so a pause can
        // freeze it. `paused`/`remaining_millis` carry the suspended state. The project foreign key
        // cascades; launch reconciliation clears whatever a previous run left (per-run owner ids).
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS timers (
                 id                   INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id           INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 owner                INTEGER NOT NULL,
                 body                 TEXT NOT NULL,
                 fire                 TEXT NOT NULL,
                 deadline_unix_millis INTEGER NOT NULL,
                 paused               INTEGER NOT NULL DEFAULT 0,
                 remaining_millis     INTEGER
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 6 {
        // Coordination scratchpads: durable, project-scoped shared documents. The store-assigned
        // `id` is durable and never reused (`AUTOINCREMENT`); `doc` is the JSON of the disciplined
        // `ScratchpadDoc` and `tags` a JSON array, so the persisted shape cannot drift; `revision`
        // guards optimistic-concurrency writes; `(project_id, name)` is unique (the addressing
        // handle). Unlike leases and timers these are NOT process-owned and are NOT cleared on
        // launch — a scratchpad survives an app restart. The project foreign key cascades.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS scratchpads (
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
        .map_err(sql_err)?;
    }

    if version < 7 {
        // Coordination todos: durable, project-scoped work items. The store-assigned `id` is durable
        // and never reused (`AUTOINCREMENT`); `doc` is the JSON of the disciplined `TodoDoc` (title,
        // description, acceptance criteria, risks, status), and `tags`/`blockers`/`comments` are JSON
        // arrays, so the persisted shapes cannot drift; `revision` guards optimistic-concurrency doc
        // writes. `locked_by` is the per-run process id holding the todo's lock, or NULL — the only
        // process-owned, per-run field, cleared on launch (the todo itself survives, G11). The
        // project foreign key cascades, so removing a project drops its todos.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
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
        .map_err(sql_err)?;
    }

    if version < 8 {
        // Coordination kv: one row per (project, key). `value` stores arbitrary JSON — the TEXT
        // column holds the serialized form; parsing happens at the repository boundary. Durable,
        // not process-owned, and survives an app restart; no launch-reconcile clear. The project
        // foreign key cascades, so removing a project drops its kv entries.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv (
                 project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 key        TEXT NOT NULL,
                 value      TEXT NOT NULL,
                 PRIMARY KEY (project_id, key)
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 9 {
        // Application settings: a single global record (the `id = 1` CHECK enforces the singleton),
        // not project-scoped, so it has no project foreign key and survives across projects. `doc`
        // is the JSON of the `Settings` document, so the persisted shape is the domain type and
        // cannot drift; serde defaults fill any field a newer build adds. Durable, never cleared on
        // launch.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                 id  INTEGER PRIMARY KEY CHECK (id = 1),
                 doc TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 10 {
        // Per-project local settings: one row per project, keyed by `project_id`, holding the
        // `ProjectSettings` document as JSON so the persisted shape is the domain type and cannot
        // drift (serde defaults fill any field a newer build adds). The project foreign key
        // cascades, so removing a project drops its local settings; durable, never cleared on
        // launch. Stored apart from the project's shared `solo.yml` config (C1) — never merged.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_settings (
                 project_id INTEGER PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
                 doc        TEXT NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 11 {
        // Feedback: append-only local notes about Soloist itself. Global (no project foreign
        // key — feedback outlives any project) and never transmitted anywhere; the user reads
        // it back at their own pace. The store-assigned `id` is durable and never reused
        // (`AUTOINCREMENT`); the millis are a persistable wall clock.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS feedback (
                 id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                 message               TEXT NOT NULL,
                 submitted_unix_millis INTEGER NOT NULL
             );",
        )
        .map_err(sql_err)?;
    }

    if version < 12 {
        // Prompt templates: durable reusable prompts, global (NULL project_id) or
        // project-scoped. `revision` guards optimistic-concurrency writes like scratchpads.
        // Name uniqueness per scope is the expression index below, NOT a UNIQUE constraint:
        // SQLite treats NULLs as distinct inside UNIQUE, so `UNIQUE(project_id, name)` would
        // allow unlimited same-named global rows. COALESCE maps the global scope to 0, which
        // no project row ever uses (rowids start at 1). The project foreign key cascades;
        // global rows outlive every project.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS prompt_templates (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id  INTEGER REFERENCES projects(id) ON DELETE CASCADE,
                 name        TEXT NOT NULL,
                 description TEXT,
                 body        TEXT NOT NULL,
                 revision    INTEGER NOT NULL
             );
             CREATE UNIQUE INDEX IF NOT EXISTS prompt_templates_scope_name
                 ON prompt_templates (COALESCE(project_id, 0), name);",
        )
        .map_err(sql_err)?;
    }

    if version < 13 {
        // Scratchpad and todo documents go free-form: a scratchpad `doc` becomes its raw Markdown
        // body and a todo `doc` becomes `{title, body, status}`. The former structured JSON is
        // converted in place, laid out as Markdown sections so no field is lost. Idempotent (a body
        // already converted is left untouched), like every step here.
        crate::doc_to_markdown::convert(conn)?;
    }

    if version < 14 {
        // Generalize `prompt_templates` into the unified `templates` table: add a `kind` column
        // (existing rows are prompts — the DEFAULT backfills them), and re-key uniqueness on
        // (kind, scope, name) so the same name may exist as a prompt, a scratchpad shape, and a
        // todo shape. Guarded so a re-run after a partial failure is a no-op, like every step here:
        // the rename runs only while the old table exists and the new one does not.
        if table_exists(conn, "prompt_templates")? && !table_exists(conn, "templates")? {
            conn.execute_batch(
                "ALTER TABLE prompt_templates RENAME TO templates;
                 ALTER TABLE templates ADD COLUMN kind TEXT NOT NULL DEFAULT 'prompt';
                 DROP INDEX IF EXISTS prompt_templates_scope_name;
                 CREATE UNIQUE INDEX IF NOT EXISTS templates_kind_scope_name
                     ON templates (kind, COALESCE(project_id, 0), name);",
            )
            .map_err(sql_err)?;
        }
    }

    if version < 15 {
        // A scratchpad gains an `updated_at` wall clock (unix millis of its last body write), so the
        // list can be ordered by recency, not only by name. Existing rows backfill to 0 — their last
        // edit time is unknown, so a recency sort lists them oldest — and are stamped on the next
        // write. Guarded on the table existing (created at v6, so always present in a real chain) and
        // the column's absence, so a re-run after a partial failure is a no-op.
        if table_exists(conn, "scratchpads")? && !column_exists(conn, "scratchpads", "updated_at")?
        {
            conn.execute_batch(
                "ALTER TABLE scratchpads ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;",
            )
            .map_err(sql_err)?;
        }
    }

    if version < 16 {
        // A todo gains an optional association with the scratchpad it was derived from. The column
        // holds the scratchpad's durable id, so a rename never breaks the link, and `ON DELETE SET
        // NULL` unlinks the todo when that document is deleted rather than leaving it pointing at a
        // row that is gone. Existing todos stay NULL — an association can only be stated, never
        // inferred, and being unlinked is a permanently valid state. Guarded on the table existing
        // (created at v7) and the column's absence, so a re-run after a partial failure is a no-op.
        if table_exists(conn, "todos")? && !column_exists(conn, "todos", "scratchpad_id")? {
            conn.execute_batch(
                "ALTER TABLE todos ADD COLUMN scratchpad_id INTEGER NULL
                     REFERENCES scratchpads(id) ON DELETE SET NULL;",
            )
            .map_err(sql_err)?;
        }
    }

    if version < 17 {
        // `projects.id` gains `AUTOINCREMENT`, so a project id is never reused. Without it SQLite
        // assigns `max(rowid) + 1`, which hands the id of a removed highest-id project to the next
        // project opened — and any in-memory state keyed by `ProjectId` then answers for the new
        // project with the removed one's data. Every other durable id here is already
        // `AUTOINCREMENT` for the same reason. The column cannot be altered in place, so the table
        // is rebuilt.
        rebuild_projects_with_autoincrement(conn)?;
    }

    if version < SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(sql_err)?;
    }
    Ok(())
}

/// Rebuilds `projects` with an `AUTOINCREMENT` primary key, preserving every row's id so no
/// foreign key that references it is orphaned.
///
/// Follows SQLite's documented table-rebuild procedure: foreign keys are disabled first (so the
/// `DROP` does not cascade every project-scoped row away, and so the `RENAME` does not rewrite the
/// child tables' `REFERENCES projects` clauses to the temporary name), the swap runs in one
/// transaction, and `foreign_key_check` verifies nothing was orphaned before it commits. The
/// pragma is restored either way. Copying the ids also seeds `sqlite_sequence` to the current
/// maximum, so the high-water mark carries over rather than restarting.
fn rebuild_projects_with_autoincrement(conn: &Connection) -> Result<(), StoreError> {
    if projects_id_autoincrements(conn)? {
        return Ok(());
    }
    let foreign_keys: bool = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(sql_err)?;
    // A pragma is a no-op inside a transaction, so this must precede the `BEGIN`.
    conn.pragma_update(None, "foreign_keys", false)
        .map_err(sql_err)?;
    let rebuilt = swap_in_rebuilt_projects(conn);
    conn.pragma_update(None, "foreign_keys", foreign_keys)
        .map_err(sql_err)?;
    rebuilt
}

/// The transactional half of the rebuild: build the replacement, copy the rows, swap the names,
/// and commit only once the foreign keys still resolve. Any failure rolls the whole swap back, so
/// the next run sees the original table and retries.
fn swap_in_rebuilt_projects(conn: &Connection) -> Result<(), StoreError> {
    conn.execute_batch(
        "BEGIN;
         DROP TABLE IF EXISTS projects_rebuilt;
         CREATE TABLE projects_rebuilt (
             id   INTEGER PRIMARY KEY AUTOINCREMENT,
             root TEXT NOT NULL UNIQUE,
             name TEXT,
             icon TEXT
         );
         INSERT INTO projects_rebuilt (id, root, name, icon)
             SELECT id, root, name, icon FROM projects;
         DROP TABLE projects;
         ALTER TABLE projects_rebuilt RENAME TO projects;",
    )
    .map_err(sql_err)?;
    match orphaned_rows(conn) {
        Ok(0) => conn.execute_batch("COMMIT;").map_err(sql_err),
        outcome => {
            let _ = conn.execute_batch("ROLLBACK;");
            match outcome {
                Ok(orphans) => Err(StoreError::Backend(format!(
                    "rebuilding projects would orphan {orphans} referencing row(s)"
                ))),
                Err(err) => Err(err),
            }
        }
    }
}

/// How many rows in the database reference a row that is not there.
fn orphaned_rows(conn: &Connection) -> Result<i64, StoreError> {
    conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
        row.get(0)
    })
    .map_err(sql_err)
}

/// Whether `projects` already declares an `AUTOINCREMENT` id — the guard that keeps the rebuild a
/// no-op on a re-run, like the other guarded steps. The declaration is only recoverable from the
/// stored `CREATE TABLE` text; no pragma reports it.
fn projects_id_autoincrements(conn: &Connection) -> Result<bool, StoreError> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'projects'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_err)?;
    Ok(sql.is_some_and(|sql| sql.to_uppercase().contains("AUTOINCREMENT")))
}

/// Whether a table of `name` exists — used by the guarded rename in the v14 step so it stays a
/// no-op on a re-run.
fn table_exists(conn: &Connection, name: &str) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .map(|found| found.is_some())
    .map_err(sql_err)
}

/// Whether `table` has a column named `column` — used by the guarded `ADD COLUMN` steps (SQLite has
/// no `ADD COLUMN IF NOT EXISTS`) so each stays a no-op on a re-run. `table` is a code
/// literal here, never caller input, so interpolating it into the `PRAGMA` is safe.
fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, StoreError> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(sql_err)?;
    let mut names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sql_err)?;
    names
        .try_fold(false, |found, name| Ok(found || name? == column))
        .map_err(sql_err)
}

/// Seeds the built-in agent providers into a fresh `agent_tools` table, preserving their
/// canonical order via `position`. The definition is the tool's JSON, so the persisted shape
/// is exactly the domain type and cannot drift from it. `INSERT OR IGNORE` keeps the step
/// idempotent and never clobbers a tool the user has since edited under the same name.
fn seed_builtin_agent_tools(conn: &Connection) -> Result<(), StoreError> {
    for (position, tool) in AgentTool::builtin_defaults().iter().enumerate() {
        let definition = serde_json::to_string(tool)
            .map_err(|err| StoreError::Backend(format!("serialize agent tool: {err}")))?;
        conn.execute(
            "INSERT OR IGNORE INTO agent_tools (name, position, definition) VALUES (?1, ?2, ?3)",
            (&tool.name, position as i64, &definition),
        )
        .map_err(sql_err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
    fn a_pre_v17_project_keeps_its_id_and_its_dependent_rows_and_stops_reusing_ids() {
        use soloist_core::{ProjectId, ProjectRepo};

        // A v16 database whose `projects` table has the plain `INTEGER PRIMARY KEY` every build
        // before this one wrote, populated across two of the tables that reference it.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("pre-v17.db");
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE projects (
                 id   INTEGER PRIMARY KEY,
                 root TEXT NOT NULL UNIQUE,
                 name TEXT,
                 icon TEXT
             );
             CREATE TABLE trust (
                 project_id   INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 variant_hash TEXT NOT NULL,
                 PRIMARY KEY (project_id, variant_hash)
             );
             CREATE TABLE kv (
                 project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 key        TEXT NOT NULL,
                 value      TEXT NOT NULL,
                 PRIMARY KEY (project_id, key)
             );
             INSERT INTO projects (id, root, name) VALUES (1, '/p/a', 'a'), (7, '/p/b', 'b');
             INSERT INTO trust (project_id, variant_hash) VALUES (7, 'abc');
             INSERT INTO kv (project_id, key, value) VALUES (7, 'k', '1');",
        )
        .expect("seed the v16 schema");
        conn.pragma_update(None, "user_version", 16)
            .expect("mark it as a v16 database");
        drop(conn);

        let store = crate::SqliteStore::open(&path).expect("upgrade and open");

        // Every project keeps the id its rows already reference — a rebuild that renumbered them
        // would silently repoint every dependent row.
        let roots: Vec<(u64, String)> = store
            .list()
            .expect("list projects")
            .into_iter()
            .map(|record| (record.id.get(), record.root.display().to_string()))
            .collect();
        assert_eq!(
            roots,
            vec![(7, "/p/b".to_owned()), (1, "/p/a".to_owned())],
            "the rebuild preserves each project's durable id"
        );

        // ...and the rows that reference them survive: the drop-and-rename must not cascade.
        {
            let conn = store.lock();
            let trust: i64 = conn
                .query_row("SELECT COUNT(*) FROM trust WHERE project_id = 7", [], |r| {
                    r.get(0)
                })
                .expect("count trust rows");
            let kv: i64 = conn
                .query_row("SELECT COUNT(*) FROM kv WHERE project_id = 7", [], |r| {
                    r.get(0)
                })
                .expect("count kv rows");
            assert_eq!(trust, 1, "a dependent trust row survives the rebuild");
            assert_eq!(kv, 1, "a dependent kv row survives the rebuild");
            // Foreign keys are enforced again once the rebuild is done, not left disabled.
            let enforcing: bool = conn
                .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
                .expect("read foreign_keys");
            assert!(enforcing, "the rebuild restores foreign-key enforcement");
        }

        // The point of the rebuild: removing the highest-id project must not free its id for the
        // next one. Before v17 the new project would have been handed id 7 and inherited every
        // in-memory cache entry keyed by it.
        store.remove(ProjectId::from_raw(7)).expect("remove b");
        let fresh = store
            .upsert(std::path::Path::new("/p/c"), None, None)
            .expect("open a new project");
        assert!(
            fresh.id.get() > 7,
            "a removed project's id is never handed to the next project, but the new project \
             took id {}",
            fresh.id.get()
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
}
