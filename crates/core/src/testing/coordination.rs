//! In-memory coordination repository fakes mirroring the SQLite store's semantics closely enough
//! to exercise the lease and timer aggregates (atomic acquire/claim, TTL/deadline handling,
//! owner-close release, launch reconcile) headless — no real database. Each is keyed exactly as
//! its durable table is, and every method holds the map lock for its whole operation so a
//! check-and-write is atomic, matching the store's contract.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::coordination::{
    LockRepo, NewTimer, RenameResult, ScratchpadDoc, ScratchpadRepo, StoredLease, StoredScratchpad,
    StoredTimer, TimerRepo, TimerStatus, WriteResult,
};
use crate::ids::{ProcessId, ProjectId, ScratchpadId, TimerId};
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

/// An in-memory [`ScratchpadRepo`] for headless coordination tests. Keyed by `(project, name)` like
/// the durable table, assigns durable ids from a counter, and mirrors the store's atomic
/// revision-guarded write, rename uniqueness, and tag read-modify-write under one lock.
#[derive(Default)]
pub struct FakeScratchpadRepo {
    rows: Mutex<HashMap<(u64, String), StoredScratchpad>>,
    next_id: AtomicU64,
}

impl FakeScratchpadRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ScratchpadRepo for FakeScratchpadRepo {
    fn write(
        &self,
        project: ProjectId,
        name: &str,
        doc: &ScratchpadDoc,
        expected: Option<u64>,
    ) -> Result<WriteResult, StoreError> {
        let mut rows = lock(&self.rows);
        let slot = (project.get(), name.to_owned());
        match rows.get(&slot) {
            Some(existing) => match expected {
                Some(rev) if rev == existing.revision => {
                    let mut updated = existing.clone();
                    updated.doc = doc.clone();
                    updated.revision = existing.revision + 1;
                    rows.insert(slot, updated.clone());
                    Ok(WriteResult::Written(Box::new(updated)))
                }
                _ => Ok(WriteResult::Conflict {
                    actual: Some(existing.revision),
                }),
            },
            None => match expected {
                None => {
                    let id =
                        ScratchpadId::from_raw(self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
                    let stored = StoredScratchpad {
                        id,
                        project,
                        name: name.to_owned(),
                        doc: doc.clone(),
                        tags: Vec::new(),
                        archived: false,
                        revision: 1,
                    };
                    rows.insert(slot, stored.clone());
                    Ok(WriteResult::Written(Box::new(stored)))
                }
                Some(_) => Ok(WriteResult::Conflict { actual: None }),
            },
        }
    }

    fn read(&self, project: ProjectId, name: &str) -> Result<Option<StoredScratchpad>, StoreError> {
        Ok(lock(&self.rows)
            .get(&(project.get(), name.to_owned()))
            .cloned())
    }

    fn list(&self, project: ProjectId) -> Result<Vec<StoredScratchpad>, StoreError> {
        let mut found: Vec<StoredScratchpad> = lock(&self.rows)
            .values()
            .filter(|row| row.project == project)
            .cloned()
            .collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(found)
    }

    fn rename(&self, project: ProjectId, from: &str, to: &str) -> Result<RenameResult, StoreError> {
        let mut rows = lock(&self.rows);
        let to_slot = (project.get(), to.to_owned());
        if from != to && rows.contains_key(&to_slot) {
            return Ok(RenameResult::NameTaken);
        }
        match rows.remove(&(project.get(), from.to_owned())) {
            Some(mut stored) => {
                stored.name = to.to_owned();
                rows.insert(to_slot, stored.clone());
                Ok(RenameResult::Renamed(Box::new(stored)))
            }
            None => Ok(RenameResult::NotFound),
        }
    }

    fn add_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                for tag in tags {
                    if !stored.tags.contains(tag) {
                        stored.tags.push(tag.clone());
                    }
                }
                stored.tags.sort();
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn remove_tags(
        &self,
        project: ProjectId,
        name: &str,
        tags: &[String],
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                stored.tags.retain(|tag| !tags.contains(tag));
                stored.tags.sort();
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn tags(&self, project: ProjectId) -> Result<Vec<String>, StoreError> {
        let distinct: BTreeSet<String> = lock(&self.rows)
            .values()
            .filter(|row| row.project == project)
            .flat_map(|row| row.tags.iter().cloned())
            .collect();
        Ok(distinct.into_iter().collect())
    }

    fn set_archived(
        &self,
        project: ProjectId,
        name: &str,
        archived: bool,
    ) -> Result<Option<StoredScratchpad>, StoreError> {
        let mut rows = lock(&self.rows);
        match rows.get_mut(&(project.get(), name.to_owned())) {
            Some(stored) => {
                stored.archived = archived;
                Ok(Some(stored.clone()))
            }
            None => Ok(None),
        }
    }

    fn delete(&self, project: ProjectId, name: &str) -> Result<bool, StoreError> {
        Ok(lock(&self.rows)
            .remove(&(project.get(), name.to_owned()))
            .is_some())
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
