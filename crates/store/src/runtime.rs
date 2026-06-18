//! The file-backed runtime-state adapter.
//!
//! The record of currently-running process groups is kept outside SQLite — it is
//! ephemeral runtime state, rebuilt each run, read once on launch to reconcile orphans.
//! Records are mirrored in memory behind one lock so concurrent process actors
//! serialize their updates, and each change is persisted atomically (temp file +
//! rename). A missing or corrupt file is treated as empty rather than failing.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use soloist_core::{OrphanRecord, RuntimeState, RuntimeStateError};

use crate::data_dir;

/// The runtime-state file name within the data directory.
const FILE: &str = "runtime-state.json";

/// Records running process groups in a small JSON file in the data directory.
pub struct FileRuntimeState {
    path: PathBuf,
    state: Mutex<Vec<OrphanRecord>>,
}

impl FileRuntimeState {
    /// Opens the runtime-state file in the resolved data directory, loading any records
    /// a previous run left behind.
    pub fn open_default() -> Result<Self, RuntimeStateError> {
        let path = data_dir()
            .map_err(|err| RuntimeStateError::Backend(err.to_string()))?
            .join(FILE);
        Ok(Self::at(path))
    }

    /// Opens the runtime-state file at `path`, loading its current contents.
    pub fn at(path: PathBuf) -> Self {
        let state = Mutex::new(read(&path));
        Self { path, state }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<OrphanRecord>> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn persist(&self, records: &[OrphanRecord]) -> Result<(), RuntimeStateError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(backend)?;
        }
        let json = serde_json::to_vec_pretty(records).map_err(backend)?;
        // Write to a temp file then rename, so a crash mid-write cannot corrupt the file.
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, &json).map_err(backend)?;
        std::fs::rename(&tmp, &self.path).map_err(backend)?;
        Ok(())
    }
}

impl RuntimeState for FileRuntimeState {
    fn record(&self, record: &OrphanRecord) -> Result<(), RuntimeStateError> {
        let mut state = self.lock();
        state.retain(|r| r.pgid != record.pgid);
        state.push(record.clone());
        self.persist(&state)
    }

    fn forget(&self, pgid: i32) -> Result<(), RuntimeStateError> {
        let mut state = self.lock();
        let before = state.len();
        state.retain(|r| r.pgid != pgid);
        if state.len() == before {
            return Ok(()); // nothing changed; skip a needless write
        }
        self.persist(&state)
    }

    fn load(&self) -> Result<Vec<OrphanRecord>, RuntimeStateError> {
        Ok(self.lock().clone())
    }
}

/// Reads and parses the file, returning an empty set if it is absent or unreadable.
fn read(path: &Path) -> Vec<OrphanRecord> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn backend<E: std::fmt::Display>(err: E) -> RuntimeStateError {
    RuntimeStateError::Backend(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn rec(pgid: i32) -> OrphanRecord {
        OrphanRecord {
            project_root: PathBuf::from("/p"),
            name: "Web".into(),
            command: "npm run dev".into(),
            pgid,
        }
    }

    #[test]
    fn records_persist_across_reopen_and_forget_removes() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join(FILE);
        {
            let rt = FileRuntimeState::at(path.clone());
            rt.record(&rec(1)).expect("record 1");
            rt.record(&rec(2)).expect("record 2");
            rt.forget(1).expect("forget 1");
        }
        // A fresh handle sees what was persisted: only pgid 2 remains.
        let loaded = FileRuntimeState::at(path).load().expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].pgid, 2);
    }

    #[test]
    fn a_missing_file_loads_empty() {
        let dir = tempdir().expect("temp dir");
        let rt = FileRuntimeState::at(dir.path().join("absent.json"));
        assert!(rt.load().expect("load").is_empty());
    }
}
