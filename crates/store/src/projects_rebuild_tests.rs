use rusqlite::Connection;

use super::*;

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
fn a_failed_rebuild_rolls_back_rather_than_leaving_its_transaction_open() {
    let conn = Connection::open_in_memory().expect("in-memory db");
    // A *view* occupying the scratch name the swap wants: `DROP TABLE` refuses to drop a view, so
    // the batch fails on its first statement with the `BEGIN` already taken. That is the shape of
    // any mid-swap failure — a full disk, a lock, a stray object — reproduced deterministically.
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         CREATE TABLE projects (
             id   INTEGER PRIMARY KEY,
             root TEXT NOT NULL UNIQUE,
             name TEXT,
             icon TEXT
         );
         INSERT INTO projects (id, root) VALUES (1, '/p/a');
         CREATE VIEW projects_rebuilt AS SELECT 1;",
    )
    .expect("seed a v16 database the rebuild cannot swap");

    let outcome = rebuild_projects_with_autoincrement(&conn);
    assert!(
        outcome.is_err(),
        "a swap that cannot run must report the failure"
    );

    // The consequence that matters. The caller restores `foreign_keys` after the swap, and a
    // pragma inside an open transaction is silently ignored — so a rebuild that failed without
    // rolling back leaves foreign keys *disabled* on a connection the app goes on using.
    let enforcing: bool = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .expect("read foreign_keys");
    assert!(
        enforcing,
        "a failed rebuild must leave foreign-key enforcement on, not silently disabled"
    );

    // And nothing of ours is still open: SQLite refuses a nested `BEGIN`, so this succeeding is
    // proof the transaction was closed rather than abandoned.
    conn.execute_batch("BEGIN; COMMIT;")
        .expect("a failed rebuild must leave no transaction open");

    // The original table is untouched, so the next startup retries the rebuild from a clean state.
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
        .expect("count projects");
    assert_eq!(rows, 1, "the original table survives a failed rebuild");
}
