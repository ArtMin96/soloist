//! In-memory fakes for orphan reconciliation: a [`FakeRuntimeState`] standing in for the
//! runtime-state file (records upserted by pgid) and a [`FakeOrphanControl`] whose
//! liveness set a test seeds and whose `signal` reaps a group, so adoption, surfacing,
//! and pruning can be exercised without real processes.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::ports::{OrphanControl, OrphanRecord, RuntimeState, RuntimeStateError, SpawnError};
use crate::sync::lock;

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

/// An in-memory [`OrphanControl`]: a test marks pgids alive, and signalling reaps the
/// group (removing it from the live set) so an adopted process's liveness poll sees it
/// die. Records the signals sent for assertions.
#[derive(Clone, Default)]
pub struct FakeOrphanControl {
    alive: Arc<Mutex<HashSet<i32>>>,
    signalled: Arc<Mutex<Vec<(i32, bool)>>>,
}

impl FakeOrphanControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a process group alive, as if left running by a previous run.
    pub fn set_alive(&self, pgid: i32) {
        lock(&self.alive).insert(pgid);
    }

    /// The signals sent, as `(pgid, force)` where `force` is SIGKILL vs SIGTERM.
    pub fn signalled(&self) -> Vec<(i32, bool)> {
        lock(&self.signalled).clone()
    }
}

impl OrphanControl for FakeOrphanControl {
    fn is_alive(&self, pgid: i32) -> bool {
        lock(&self.alive).contains(&pgid)
    }

    fn signal(&self, pgid: i32, force: bool) -> Result<(), SpawnError> {
        lock(&self.signalled).push((pgid, force));
        lock(&self.alive).remove(&pgid);
        Ok(())
    }
}
