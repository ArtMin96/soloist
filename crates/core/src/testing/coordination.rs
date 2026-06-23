//! An in-memory [`LockRepo`] fake mirroring the SQLite store's semantics closely enough to
//! exercise the lease aggregate (TTL expiry, owner-close release, launch reconcile) headless —
//! no real database. Keyed by `(project, key)`, exactly as the durable table is.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::coordination::{LockRepo, StoredLease};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::StoreError;
use crate::sync::lock;

/// An in-memory [`LockRepo`] for headless coordination tests.
#[derive(Default)]
pub struct FakeLockRepo {
    rows: Mutex<HashMap<(u64, String), StoredLease>>,
}

impl FakeLockRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl LockRepo for FakeLockRepo {
    fn get(&self, project: ProjectId, key: &str) -> Result<Option<StoredLease>, StoreError> {
        Ok(lock(&self.rows)
            .get(&(project.get(), key.to_owned()))
            .cloned())
    }

    fn put(&self, lease: &StoredLease) -> Result<(), StoreError> {
        lock(&self.rows).insert((lease.project.get(), lease.key.clone()), lease.clone());
        Ok(())
    }

    fn remove(&self, project: ProjectId, key: &str) -> Result<bool, StoreError> {
        Ok(lock(&self.rows)
            .remove(&(project.get(), key.to_owned()))
            .is_some())
    }

    fn release_owner(&self, owner: ProcessId) -> Result<usize, StoreError> {
        let mut rows = lock(&self.rows);
        let before = rows.len();
        rows.retain(|_, lease| lease.owner != owner);
        Ok(before - rows.len())
    }

    fn clear(&self) -> Result<usize, StoreError> {
        let mut rows = lock(&self.rows);
        let cleared = rows.len();
        rows.clear();
        Ok(cleared)
    }
}
