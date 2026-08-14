//! Watch vocabulary: why a directory could not be watched.
//!
//! Shared kernel, not a context. The [`FileWatcher`](crate::filewatch::FileWatcher) port returns
//! it, the git context's watch consumes it, and [`crate::events`] carries it to the surfaces — so
//! like [`crate::idle`]'s activity and [`crate::orphans`]' report, it is owned by none of them and
//! depends on nothing. The logic that meets these refusals lives in the reactors that establish
//! the watches.

use serde::Serialize;
use thiserror::Error;

/// Why a directory could not be watched.
///
/// A closed set, because the answer differs by case: an exhausted budget is the user's to raise and
/// is worth saying so about, while a path that is not there may simply have gone. A watch that
/// cannot be established is reported rather than swallowed — a watch that silently yields no events
/// is indistinguishable from a tree nothing ever changes in, which is the one failure nobody
/// notices.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Error, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchError {
    /// The OS refused another watch because the per-user file-watch budget is exhausted. On Linux
    /// that is `fs.inotify.max_user_watches`, shared with every other program on the machine, and a
    /// recursive watch spends one per directory beneath its root.
    #[error("the system's file-watch limit is exhausted")]
    BudgetExhausted,
    /// The path itself could not be watched: it does not exist, is not readable, or vanished while
    /// the watch was being established.
    #[error("the directory could not be watched")]
    Unwatchable,
    /// The watching backend could not be started at all, so nothing under this root will report.
    #[error("the filesystem watcher is unavailable")]
    Unavailable,
}
