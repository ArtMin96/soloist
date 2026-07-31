//! The project registry repository — the core [`ProjectRepo`] port.

use std::path::PathBuf;

use rusqlite::{OptionalExtension, Row};
use soloist_core::{ProjectId, ProjectRecord, ProjectRepo, StoreError};

use crate::{path_str, sql_err, SqliteStore};

impl ProjectRepo for SqliteStore {
    fn upsert(
        &self,
        root: &std::path::Path,
        name: Option<&str>,
        icon: Option<&std::path::Path>,
    ) -> Result<ProjectRecord, StoreError> {
        let root_str = path_str(root)?;
        let icon_str = icon.map(path_str).transpose()?;

        let conn = self.lock();
        // `position` is left to a reorder alone: a project opens unarranged, and the reader's
        // `id DESC` tiebreak leads the list with it. Re-opening a known project refreshes only
        // its metadata, so one the user has placed by hand keeps its place.
        conn.execute(
            "INSERT INTO projects (root, name, icon) VALUES (?1, ?2, ?3)
             ON CONFLICT(root) DO UPDATE SET name = excluded.name, icon = excluded.icon",
            (root_str, name, icon_str),
        )
        .map_err(sql_err)?;
        let id: i64 = conn
            .query_row(
                "SELECT id FROM projects WHERE root = ?1",
                [root_str],
                |row| row.get(0),
            )
            .map_err(sql_err)?;

        Ok(ProjectRecord {
            id: ProjectId::from_raw(id as u64),
            root: root.to_path_buf(),
            name: name.map(str::to_owned),
            icon: icon.map(std::path::Path::to_path_buf),
        })
    }

    fn list(&self) -> Result<Vec<ProjectRecord>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id, root, name, icon FROM projects ORDER BY position, id DESC")
            .map_err(sql_err)?;
        let rows = stmt.query_map([], row_to_record).map_err(sql_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(sql_err)
    }

    fn get(&self, id: ProjectId) -> Result<Option<ProjectRecord>, StoreError> {
        self.lock()
            .query_row(
                "SELECT id, root, name, icon FROM projects WHERE id = ?1",
                [id.get() as i64],
                row_to_record,
            )
            .optional()
            .map_err(sql_err)
    }

    fn reorder(&self, order: &[ProjectId]) -> Result<(), StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT id FROM projects ORDER BY position, id DESC")
            .map_err(sql_err)?;
        let known: Vec<i64> = stmt
            .query_map([], |row| row.get(0))
            .map_err(sql_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql_err)?;
        drop(stmt);

        // The named ids lead, in the order given; whatever the caller did not name keeps its
        // current relative order behind them. An id naming no project simply updates no row.
        let named: Vec<i64> = order.iter().map(|id| id.get() as i64).collect();
        let placed: Vec<i64> = named
            .iter()
            .copied()
            .chain(known.into_iter().filter(|id| !named.contains(id)))
            .collect();

        // One transaction, so a failure part-way through cannot leave the list in an order the
        // user never arranged.
        let tx = conn.unchecked_transaction().map_err(sql_err)?;
        for (position, id) in placed.iter().enumerate() {
            tx.execute(
                "UPDATE projects SET position = ?1 WHERE id = ?2",
                (position as i64, id),
            )
            .map_err(sql_err)?;
        }
        tx.commit().map_err(sql_err)
    }

    fn remove(&self, id: ProjectId) -> Result<(), StoreError> {
        self.lock()
            .execute("DELETE FROM projects WHERE id = ?1", [id.get() as i64])
            .map(|_| ())
            .map_err(sql_err)
    }
}

fn row_to_record(row: &Row) -> rusqlite::Result<ProjectRecord> {
    let id: i64 = row.get(0)?;
    let root: String = row.get(1)?;
    let name: Option<String> = row.get(2)?;
    let icon: Option<String> = row.get(3)?;
    Ok(ProjectRecord {
        id: ProjectId::from_raw(id as u64),
        root: PathBuf::from(root),
        name,
        icon: icon.map(PathBuf::from),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteStore;
    use tempfile::tempdir;

    #[test]
    fn upsert_assigns_a_durable_id_and_updates_metadata() {
        let dir = tempdir().expect("temp dir");
        let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
        let root = std::path::Path::new("/projects/storefront");

        let first = store
            .upsert(root, Some("storefront"), None)
            .expect("insert");
        let again = store.upsert(root, Some("renamed"), None).expect("update");
        assert_eq!(first.id, again.id, "same root keeps the same durable id");
        assert_eq!(again.name.as_deref(), Some("renamed"));
        assert_eq!(store.list().expect("list").len(), 1);
    }

    #[test]
    fn ids_are_stable_across_reopen() {
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        let id = {
            let store = SqliteStore::open(&db).expect("open");
            store
                .upsert(std::path::Path::new("/projects/app"), None, None)
                .expect("insert")
                .id
        };
        let reopened = SqliteStore::open(&db).expect("reopen");
        let got = reopened
            .get(id)
            .expect("get")
            .expect("project survives reopen");
        assert_eq!(got.root, PathBuf::from("/projects/app"));
    }

    /// Opens the named roots in order and returns the store beside their ids.
    fn store_with(roots: &[&str]) -> (tempfile::TempDir, SqliteStore, Vec<ProjectId>) {
        let dir = tempdir().expect("temp dir");
        let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
        let ids = roots
            .iter()
            .map(|root| {
                store
                    .upsert(std::path::Path::new(root), None, None)
                    .expect("insert")
                    .id
            })
            .collect();
        (dir, store, ids)
    }

    fn listed_ids(store: &SqliteStore) -> Vec<ProjectId> {
        store
            .list()
            .expect("list")
            .into_iter()
            .map(|record| record.id)
            .collect()
    }

    #[test]
    fn an_untouched_list_leads_with_the_most_recently_opened_project() {
        let (_dir, store, ids) = store_with(&["/p/a", "/p/b", "/p/c"]);

        assert_eq!(
            listed_ids(&store),
            vec![ids[2], ids[1], ids[0]],
            "until the user arranges the list, the newest project leads it"
        );
    }

    #[test]
    fn the_list_comes_back_in_the_order_the_user_arranged() {
        let (_dir, store, ids) = store_with(&["/p/a", "/p/b", "/p/c"]);

        let arranged = vec![ids[0], ids[2], ids[1]];
        store.reorder(&arranged).expect("reorder");

        assert_eq!(listed_ids(&store), arranged);
    }

    #[test]
    fn an_arranged_order_survives_reopening_the_database() {
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        let arranged = {
            let store = SqliteStore::open(&db).expect("open");
            let ids: Vec<ProjectId> = ["/p/a", "/p/b", "/p/c"]
                .iter()
                .map(|root| {
                    store
                        .upsert(std::path::Path::new(root), None, None)
                        .expect("insert")
                        .id
                })
                .collect();
            let arranged = vec![ids[1], ids[2], ids[0]];
            store.reorder(&arranged).expect("reorder");
            arranged
        };

        let reopened = SqliteStore::open(&db).expect("reopen");
        assert_eq!(
            listed_ids(&reopened),
            arranged,
            "the arrangement is durable, not a per-run detail"
        );
    }

    #[test]
    fn a_project_opened_after_an_upgrade_leads_the_projects_that_predate_it() {
        // The list an existing install carries into this schema: projects it has never arranged,
        // whose order the upgrade seeds rather than the user. The next project opened has to land
        // ahead of them, exactly as it would have before the column existed.
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        let conn = rusqlite::Connection::open(&db).expect("seed a pre-upgrade database");
        conn.execute_batch(
            "CREATE TABLE projects (
                 id   INTEGER PRIMARY KEY AUTOINCREMENT,
                 root TEXT NOT NULL UNIQUE,
                 name TEXT,
                 icon TEXT
             );
             INSERT INTO projects (id, root) VALUES (1, '/p/a'), (2, '/p/b');",
        )
        .expect("seed the pre-upgrade rows");
        conn.pragma_update(None, "user_version", 18)
            .expect("mark it as a pre-upgrade database");
        drop(conn);

        let store = SqliteStore::open(&db).expect("open upgrades the database");
        let fresh = store
            .upsert(std::path::Path::new("/p/c"), None, None)
            .expect("open a project after the upgrade")
            .id;

        assert_eq!(
            listed_ids(&store),
            vec![fresh, ProjectId::from_raw(2), ProjectId::from_raw(1)],
            "the upgrade keeps the order the user saw, and the next project opened still leads"
        );
    }

    #[test]
    fn a_newly_opened_project_leads_an_arranged_list() {
        let (_dir, store, ids) = store_with(&["/p/a", "/p/b"]);
        store.reorder(&[ids[0], ids[1]]).expect("reorder");

        let fresh = store
            .upsert(std::path::Path::new("/p/c"), None, None)
            .expect("insert")
            .id;

        assert_eq!(listed_ids(&store), vec![fresh, ids[0], ids[1]]);
    }

    #[test]
    fn reopening_a_project_leaves_it_where_the_user_put_it() {
        let (_dir, store, ids) = store_with(&["/p/a", "/p/b", "/p/c"]);
        let arranged = vec![ids[1], ids[2], ids[0]];
        store.reorder(&arranged).expect("reorder");

        store
            .upsert(std::path::Path::new("/p/c"), Some("renamed"), None)
            .expect("reopen with fresh metadata");

        assert_eq!(
            listed_ids(&store),
            arranged,
            "a reload refreshes metadata; it does not rearrange the list"
        );
    }

    #[test]
    fn projects_the_order_omits_keep_their_relative_order_behind_it() {
        let (_dir, store, ids) = store_with(&["/p/a", "/p/b", "/p/c", "/p/d"]);
        // The untouched list is d, c, b, a — so b and a trail in that order.
        store.reorder(&[ids[2], ids[3]]).expect("reorder");

        assert_eq!(listed_ids(&store), vec![ids[2], ids[3], ids[1], ids[0]]);
    }

    #[test]
    fn an_order_naming_a_project_that_is_gone_still_arranges_the_rest() {
        let (_dir, store, ids) = store_with(&["/p/a", "/p/b"]);
        let removed = store
            .upsert(std::path::Path::new("/p/gone"), None, None)
            .expect("insert")
            .id;
        store.remove(removed).expect("remove");

        store
            .reorder(&[ids[0], removed, ids[1]])
            .expect("a stale id does not fail the reorder");

        assert_eq!(listed_ids(&store), vec![ids[0], ids[1]]);
    }

    #[test]
    fn remove_deletes_the_project() {
        let dir = tempdir().expect("temp dir");
        let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
        let a = store
            .upsert(std::path::Path::new("/p/a"), None, None)
            .expect("a");
        store
            .upsert(std::path::Path::new("/p/b"), None, None)
            .expect("b");
        store.remove(a.id).expect("remove a");
        assert!(store.get(a.id).expect("get").is_none());
        assert_eq!(store.list().expect("list").len(), 1);
    }
}
