//! An in-memory [`LockRepo`] fake mirroring the SQLite store's semantics closely enough to
//! exercise the lease aggregate (atomic acquire, TTL expiry, owner-close release, launch
//! reconcile) headless — no real database. Keyed by `(project, key)`, exactly as the durable
//! table is; each method holds the map lock for its whole operation, so a check-and-write is
//! atomic, matching the store's contract.

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
    fn acquire(
        &self,
        candidate: &StoredLease,
        now: u64,
    ) -> Result<Option<StoredLease>, StoreError> {
        let mut rows = lock(&self.rows);
        let slot = (candidate.project.get(), candidate.key.clone());
        if let Some(existing) = rows.get(&slot) {
            // A live lease (deadline strictly after `now`) owned by someone else blocks the grant.
            if existing.expires_unix_millis > now && existing.owner != candidate.owner {
                return Ok(Some(existing.clone()));
            }
        }
        rows.insert(slot, candidate.clone());
        Ok(None)
    }

    fn live(
        &self,
        project: ProjectId,
        key: &str,
        now: u64,
    ) -> Result<Option<StoredLease>, StoreError> {
        let mut rows = lock(&self.rows);
        let slot = (project.get(), key.to_owned());
        match rows.get(&slot) {
            Some(lease) if lease.expires_unix_millis > now => Ok(Some(lease.clone())),
            Some(_) => {
                rows.remove(&slot);
                Ok(None)
            }
            None => Ok(None),
        }
    }

    fn release(&self, project: ProjectId, key: &str, owner: ProcessId) -> Result<bool, StoreError> {
        let mut rows = lock(&self.rows);
        let slot = (project.get(), key.to_owned());
        match rows.get(&slot) {
            Some(lease) if lease.owner == owner => {
                rows.remove(&slot);
                Ok(true)
            }
            _ => Ok(false),
        }
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
