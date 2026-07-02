//! In-memory [`FeedbackRepo`] fake for headless support tests — no real database.

use std::sync::Mutex;

use crate::ports::StoreError;
use crate::support::{FeedbackEntry, FeedbackRepo};
use crate::sync::lock;

/// An in-memory [`FeedbackRepo`] for headless tests. Appends assign sequential ids and keep
/// insertion order, mirroring the durable store's append/list contract without SQLite.
#[derive(Default)]
pub struct FakeFeedbackRepo {
    rows: Mutex<Vec<FeedbackEntry>>,
}

impl FakeFeedbackRepo {
    pub fn new() -> Self {
        Self::default()
    }
}

impl FeedbackRepo for FakeFeedbackRepo {
    fn append(
        &self,
        message: &str,
        submitted_unix_millis: u64,
    ) -> Result<FeedbackEntry, StoreError> {
        let mut rows = lock(&self.rows);
        let entry = FeedbackEntry {
            id: rows.len() as u64 + 1,
            message: message.to_owned(),
            submitted_unix_millis,
        };
        rows.push(entry.clone());
        Ok(entry)
    }

    fn list(&self) -> Result<Vec<FeedbackEntry>, StoreError> {
        Ok(lock(&self.rows).clone())
    }

    fn count(&self) -> Result<u64, StoreError> {
        Ok(lock(&self.rows).len() as u64)
    }
}
