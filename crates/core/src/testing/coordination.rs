//! In-memory coordination repository fakes mirroring the SQLite store's semantics closely enough
//! to exercise the lease and timer aggregates (atomic acquire/claim, TTL/deadline handling,
//! owner-close release, launch reconcile) headless — no real database. Each is keyed exactly as
//! its durable table is, and every method holds the map lock for its whole operation so a
//! check-and-write is atomic, matching the store's contract.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::coordination::{LockRepo, NewTimer, StoredLease, StoredTimer, TimerRepo, TimerStatus};
use crate::ids::{ProcessId, ProjectId, TimerId};
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

    fn live_in_project(
        &self,
        project: ProjectId,
        now: u64,
    ) -> Result<Vec<StoredLease>, StoreError> {
        let rows = lock(&self.rows);
        let mut live: Vec<StoredLease> = rows
            .values()
            .filter(|lease| lease.project == project && lease.expires_unix_millis > now)
            .cloned()
            .collect();
        live.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(live)
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

/// An in-memory [`TimerRepo`] for headless coordination tests. Assigns ids from a counter (never
/// reused, like the store's `AUTOINCREMENT`) and keeps one row per timer, mirroring the durable
/// table's atomic claim/pause/resume semantics.
#[derive(Default)]
pub struct FakeTimerRepo {
    rows: Mutex<HashMap<u64, StoredTimer>>,
    next_id: AtomicU64,
}

impl FakeTimerRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TimerRepo for FakeTimerRepo {
    fn create(&self, timer: &NewTimer) -> Result<TimerId, StoreError> {
        let id = TimerId::from_raw(self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        lock(&self.rows).insert(
            id.get(),
            StoredTimer {
                id,
                project: timer.project,
                owner: timer.owner,
                body: timer.body.clone(),
                fire: timer.fire.clone(),
                deadline_unix_millis: timer.deadline_unix_millis,
                status: TimerStatus::Armed,
                remaining_on_pause_millis: None,
            },
        );
        Ok(id)
    }

    fn armed(&self) -> Result<Vec<StoredTimer>, StoreError> {
        Ok(lock(&self.rows)
            .values()
            .filter(|timer| timer.status == TimerStatus::Armed)
            .cloned()
            .collect())
    }

    fn take_if_armed(&self, id: TimerId) -> Result<Option<StoredTimer>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get(&id.get()) {
            Some(timer) if timer.status == TimerStatus::Armed => Ok(rows.remove(&id.get())),
            _ => Ok(None),
        }
    }

    fn cancel(&self, id: TimerId, owner: ProcessId) -> Result<bool, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get(&id.get()) {
            Some(timer) if timer.owner == owner => {
                rows.remove(&id.get());
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn pause(&self, id: TimerId, owner: ProcessId, now: u64) -> Result<bool, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&id.get()) {
            Some(timer) if timer.owner == owner && timer.status == TimerStatus::Armed => {
                timer.remaining_on_pause_millis =
                    Some(timer.deadline_unix_millis.saturating_sub(now));
                timer.status = TimerStatus::Paused;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn resume(&self, id: TimerId, owner: ProcessId, now: u64) -> Result<bool, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&id.get()) {
            Some(timer) if timer.owner == owner && timer.status == TimerStatus::Paused => {
                let remaining = timer.remaining_on_pause_millis.take().unwrap_or(0);
                timer.deadline_unix_millis = now.saturating_add(remaining);
                timer.status = TimerStatus::Armed;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn list_in_project(&self, project: ProjectId) -> Result<Vec<StoredTimer>, StoreError> {
        let mut timers: Vec<StoredTimer> = lock(&self.rows)
            .values()
            .filter(|timer| timer.project == project)
            .cloned()
            .collect();
        timers.sort_by_key(|timer| timer.id.get());
        Ok(timers)
    }

    fn list(&self, owner: ProcessId) -> Result<Vec<StoredTimer>, StoreError> {
        Ok(lock(&self.rows)
            .values()
            .filter(|timer| timer.owner == owner)
            .cloned()
            .collect())
    }

    fn release_owner(&self, owner: ProcessId) -> Result<usize, StoreError> {
        let mut rows = lock(&self.rows);
        let before = rows.len();
        rows.retain(|_, timer| timer.owner != owner);
        Ok(before - rows.len())
    }

    fn clear(&self) -> Result<usize, StoreError> {
        let mut rows = lock(&self.rows);
        let cleared = rows.len();
        rows.clear();
        Ok(cleared)
    }
}
