//! Locally stored feedback: an agent (or the user, through one) can leave a note about
//! Soloist. Entries are appended to the durable store and never leave the machine — there
//! is no telemetry endpoint; the user reviews them at their own pace.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::ports::{Clock, StoreError};

/// The longest accepted feedback message, in characters. Feedback is a note, not a log
/// dump.
pub const MAX_FEEDBACK_LEN: usize = 4_000;

/// The most entries the store accepts before refusing further submissions. Together with
/// [`MAX_FEEDBACK_LEN`] this bounds the table, so a runaway caller cannot grow the store
/// without bound. The check-then-append is not atomic across concurrent submitters, so the
/// ceiling may be overshot by a few in-flight entries — it is a safety bound, not a quota.
pub const MAX_FEEDBACK_ENTRIES: u64 = 500;

/// A stored feedback entry: its store-assigned id, the message, and when it was submitted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackEntry {
    pub id: u64,
    pub message: String,
    pub submitted_unix_millis: u64,
}

/// Durable, append-only feedback storage. Global — feedback is about Soloist, not a
/// project — so entries carry no project and survive everything short of deleting the
/// database.
pub trait FeedbackRepo: Send + Sync {
    /// Appends a message, returning the stored entry with its assigned id.
    fn append(
        &self,
        message: &str,
        submitted_unix_millis: u64,
    ) -> Result<FeedbackEntry, StoreError>;

    /// Every stored entry, oldest first.
    fn list(&self) -> Result<Vec<FeedbackEntry>, StoreError>;

    /// How many entries are stored.
    fn count(&self) -> Result<u64, StoreError>;
}

/// A [`FeedbackRepo`] that stores nothing — the default until the durable adapter is
/// wired. An append acknowledges without persisting; reads are empty.
#[derive(Clone, Copy, Default)]
pub struct NoopFeedbackRepo;

impl FeedbackRepo for NoopFeedbackRepo {
    fn append(
        &self,
        message: &str,
        submitted_unix_millis: u64,
    ) -> Result<FeedbackEntry, StoreError> {
        Ok(FeedbackEntry {
            id: 0,
            message: message.to_owned(),
            submitted_unix_millis,
        })
    }

    fn list(&self) -> Result<Vec<FeedbackEntry>, StoreError> {
        Ok(Vec::new())
    }

    fn count(&self) -> Result<u64, StoreError> {
        Ok(0)
    }
}

/// Why a feedback submission was refused.
#[derive(Debug, thiserror::Error)]
pub enum FeedbackError {
    #[error("feedback message is empty")]
    Empty,
    #[error("feedback message is too long (max {MAX_FEEDBACK_LEN} characters)")]
    TooLong,
    #[error("the local feedback store is full ({MAX_FEEDBACK_ENTRIES} entries); nothing more will be recorded")]
    Full,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The feedback aggregate: validates and timestamps a submission, then delegates
/// persistence to the [`FeedbackRepo`] port.
pub struct Feedback {
    repo: Arc<dyn FeedbackRepo>,
    clock: Arc<dyn Clock>,
}

impl Feedback {
    pub fn new(repo: Arc<dyn FeedbackRepo>, clock: Arc<dyn Clock>) -> Self {
        Self { repo, clock }
    }

    /// Stores `message` (trimmed) with the current wall-clock time. An empty or oversized
    /// message is refused before anything persists, and so is any submission once the store
    /// holds [`MAX_FEEDBACK_ENTRIES`].
    pub fn submit(&self, message: &str) -> Result<FeedbackEntry, FeedbackError> {
        let message = message.trim();
        if message.is_empty() {
            return Err(FeedbackError::Empty);
        }
        if message.chars().count() > MAX_FEEDBACK_LEN {
            return Err(FeedbackError::TooLong);
        }
        if self.repo.count()? >= MAX_FEEDBACK_ENTRIES {
            return Err(FeedbackError::Full);
        }
        Ok(self.repo.append(message, self.clock.now_unix_millis())?)
    }

    /// Every stored entry, oldest first.
    pub fn list(&self) -> Result<Vec<FeedbackEntry>, StoreError> {
        self.repo.list()
    }
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
