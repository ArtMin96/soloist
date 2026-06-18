//! The metadata key/value repository — the core [`Store`] port.

use soloist_core::{Store, StoreError};

use crate::{sql_err, SqliteStore};

impl Store for SqliteStore {
    fn meta_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT value FROM meta WHERE key = ?1")
            .map_err(sql_err)?;
        match stmt.query_row([key], |row| row.get::<_, String>(0)) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(sql_err(err)),
        }
    }

    fn meta_set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.lock()
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (key, value),
            )
            .map(|_| ())
            .map_err(sql_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteStore;
    use tempfile::tempdir;

    #[test]
    fn meta_round_trips_and_upserts() {
        let dir = tempdir().expect("temp dir");
        let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");

        assert_eq!(store.meta_get("schema").expect("get absent"), None);
        store.meta_set("schema", "alpha").expect("set");
        assert_eq!(
            store.meta_get("schema").expect("get present"),
            Some("alpha".to_string())
        );
        store.meta_set("schema", "beta").expect("upsert");
        assert_eq!(
            store.meta_get("schema").expect("get updated"),
            Some("beta".to_string())
        );
    }

    #[test]
    fn reopening_an_existing_db_preserves_meta() {
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        SqliteStore::open(&db)
            .expect("first open")
            .meta_set("k", "v")
            .expect("write");
        let reopened = SqliteStore::open(&db).expect("second open");
        assert_eq!(reopened.meta_get("k").expect("read"), Some("v".to_string()));
    }
}
