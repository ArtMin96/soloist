//! In-memory fakes for orphan reconciliation: a [`FakeRuntimeState`] standing in for the
//! runtime-state file (records upserted by pgid) and a [`FakeOrphanControl`] whose
//! liveness set a test seeds and whose `signal` reaps a group, so adoption, surfacing,
//! and pruning can be exercised without real processes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::ports::{
    OrphanControl, OrphanRecord, ProcessIdentity, RuntimeState, RuntimeStateError, SpawnError,
};
use crate::sync::lock;

/// The canonical identity a seeded [`OrphanRecord`] and a [`FakeOrphanControl::set_alive`]
/// group share, so a legitimate leftover matches and is adopted/surfaced. A test that
/// wants a *recycled* pgid seeds this on the record but marks the live group with a
/// different identity (a differing boot or start-time), which must read as dead.
pub fn fake_identity() -> ProcessIdentity {
    ProcessIdentity {
        boot_id: "boot-fake".into(),
        started_at: 1000,
    }
}

/// An in-memory [`RuntimeState`] standing in for the runtime-state file: records are
/// upserted by pgid, so a test can seed leftovers and assert what reconciliation
/// recorded, forgot, or pruned.
#[derive(Clone, Default)]
pub struct FakeRuntimeState {
    records: Arc<Mutex<Vec<OrphanRecord>>>,
}

impl FakeRuntimeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Pre-populates a leftover record, as if written by a previous run.
    pub fn seed(&self, record: OrphanRecord) {
        lock(&self.records).push(record);
    }

    /// The currently recorded process groups.
    pub fn records(&self) -> Vec<OrphanRecord> {
        lock(&self.records).clone()
    }
}

impl RuntimeState for FakeRuntimeState {
    fn record(&self, record: &OrphanRecord) -> Result<(), RuntimeStateError> {
        let mut records = lock(&self.records);
        records.retain(|r| r.pgid != record.pgid);
        records.push(record.clone());
        Ok(())
    }

    fn forget(&self, pgid: i32) -> Result<(), RuntimeStateError> {
        lock(&self.records).retain(|r| r.pgid != pgid);
        Ok(())
    }

    fn load(&self) -> Result<Vec<OrphanRecord>, RuntimeStateError> {
        Ok(lock(&self.records).clone())
    }
}

/// An in-memory [`OrphanControl`]: a test marks a pgid alive with a current identity,
/// and signalling reaps the group (removing it from the live map) so an adopted process's
/// liveness poll sees it die. The identity-matching rule is inherited from the port's
/// default `is_recorded_alive`, so this fake exercises the real safety logic. Records the
/// signals sent for assertions.
#[derive(Clone, Default)]
pub struct FakeOrphanControl {
    alive: Arc<Mutex<HashMap<i32, ProcessIdentity>>>,
    signalled: Arc<Mutex<Vec<(i32, bool)>>>,
    fail: Arc<Mutex<bool>>,
}

impl FakeOrphanControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every subsequent `signal` fail without reaping the group, so the failed-kill
    /// path (surface the error, keep the record) can be exercised.
    pub fn fail_signals(&self) {
        *lock(&self.fail) = true;
    }

    /// Marks a process group alive with the canonical [`fake_identity`], as if left
    /// running by a previous run whose record carries that same identity.
    pub fn set_alive(&self, pgid: i32) {
        self.set_identity(pgid, fake_identity());
    }

    /// Marks a process group alive with a specific current identity, so a test can make
    /// a live group's identity differ from a record's (a recycled pgid).
    pub fn set_identity(&self, pgid: i32, identity: ProcessIdentity) {
        lock(&self.alive).insert(pgid, identity);
    }

    /// The signals sent, as `(pgid, force)` where `force` is SIGKILL vs SIGTERM.
    pub fn signalled(&self) -> Vec<(i32, bool)> {
        lock(&self.signalled).clone()
    }
}

impl OrphanControl for FakeOrphanControl {
    fn identify(&self, pgid: i32) -> Option<ProcessIdentity> {
        lock(&self.alive).get(&pgid).cloned()
    }

    fn signal(&self, pgid: i32, force: bool) -> Result<(), SpawnError> {
        lock(&self.signalled).push((pgid, force));
        if *lock(&self.fail) {
            // The signal was attempted but failed; the group survives (stays alive).
            return Err(SpawnError::Signal("fake signal failure".into()));
        }
        lock(&self.alive).remove(&pgid);
        Ok(())
    }
}
