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
            .prepare("SELECT id, root, name, icon FROM projects ORDER BY id DESC")
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
