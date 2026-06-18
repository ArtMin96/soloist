//! The trust repository — the core [`TrustRepo`] port.
//!
//! Trust rows are keyed by `(project_id, variant_hash)`. The presence of a row means
//! that exact command variant is trusted within that project. The `project_id`
//! foreign key cascades, so removing a project drops its trust with it.

use rusqlite::OptionalExtension;
use soloist_core::{Hash, ProjectId, StoreError, TrustRepo};

use crate::{sql_err, SqliteStore};

impl TrustRepo for SqliteStore {
    fn is_trusted(&self, project: ProjectId, variant: &Hash) -> Result<bool, StoreError> {
        let found: Option<i64> = self
            .lock()
            .query_row(
                "SELECT 1 FROM trust WHERE project_id = ?1 AND variant_hash = ?2",
                (project.get() as i64, variant.to_hex()),
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_err)?;
        Ok(found.is_some())
    }

    fn set_trusted(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        self.lock()
            .execute(
                "INSERT OR IGNORE INTO trust (project_id, variant_hash) VALUES (?1, ?2)",
                (project.get() as i64, variant.to_hex()),
            )
            .map(|_| ())
            .map_err(sql_err)
    }

    fn revoke(&self, project: ProjectId, variant: &Hash) -> Result<(), StoreError> {
        self.lock()
            .execute(
                "DELETE FROM trust WHERE project_id = ?1 AND variant_hash = ?2",
                (project.get() as i64, variant.to_hex()),
            )
            .map(|_| ())
            .map_err(sql_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteStore;
    use soloist_core::{content_hash, ProjectRepo};
    use tempfile::tempdir;

    fn project_with_trust(store: &SqliteStore, root: &str) -> ProjectId {
        store
            .upsert(std::path::Path::new(root), None, None)
            .expect("project for trust fk")
            .id
    }

    #[test]
    fn trust_persists_across_reopen() {
        let dir = tempdir().expect("temp dir");
        let db = dir.path().join("soloist.db");
        let variant = content_hash(b"npm run dev|/app|");
        let project = {
            let store = SqliteStore::open(&db).expect("open");
            let project = project_with_trust(&store, "/projects/app");
            store.set_trusted(project, &variant).expect("trust");
            project
        };

        let reopened = SqliteStore::open(&db).expect("reopen");
        assert!(
            reopened.is_trusted(project, &variant).expect("query"),
            "trust must survive a restart"
        );
    }

    #[test]
    fn revoke_and_scope_behave() {
        let dir = tempdir().expect("temp dir");
        let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
        let a = project_with_trust(&store, "/p/a");
        let b = project_with_trust(&store, "/p/b");
        let variant = content_hash(b"shared command");

        store.set_trusted(a, &variant).expect("trust a");
        assert!(store.is_trusted(a, &variant).expect("a trusted"));
        assert!(
            !store.is_trusted(b, &variant).expect("b untrusted"),
            "trust is per project"
        );

        store.revoke(a, &variant).expect("revoke");
        assert!(!store.is_trusted(a, &variant).expect("a revoked"));
    }

    #[test]
    fn removing_a_project_cascades_its_trust() {
        let dir = tempdir().expect("temp dir");
        let store = SqliteStore::open(&dir.path().join("soloist.db")).expect("open");
        let project = project_with_trust(&store, "/p/cascade");
        let variant = content_hash(b"command");
        store.set_trusted(project, &variant).expect("trust");

        store.remove(project).expect("remove project");
        assert!(
            !store.is_trusted(project, &variant).expect("query"),
            "trust rows must cascade-delete with their project"
        );
    }
}
