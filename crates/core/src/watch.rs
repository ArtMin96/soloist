//! Watch vocabulary: what a watch is established for, why one could not be, what it means to be
//! granted only part of it, and what a re-sync met.
//!
//! Shared kernel, not a context. The [`FileWatcher`](crate::filewatch::FileWatcher) port returns
//! [`WatchError`], both watch reactors report their [`WatchOutcome`]s, and [`crate::events`]
//! carries [`WatchLimit`]s to the surfaces keyed by [`WatchPurpose`] — so like [`crate::idle`]'s
//! activity and [`crate::orphans`]' report, this vocabulary is owned by none of them and depends
//! on nothing but the ids. The logic that meets these limits lives in the reactors that establish
//! the watches.

use serde::Serialize;
use thiserror::Error;

use crate::ids::ProjectId;

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

/// What a watch on a project's directories was established for.
///
/// The two reactors register their own watches over the same tree, so the OS can grant one and
/// refuse the other — and the two losses are different things to be told about: one stops a
/// `restart_when_changed` command reloading on a save, the other stops a status refreshing on its
/// own. A project may also ask for only one of them, so keeping the answers apart is what lets a
/// surface name the consequence that actually followed rather than both.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchPurpose {
    /// The file-watch restart policy: a `restart_when_changed` command reloading on a save.
    Restarts,
    /// The git rail: a project's status re-read when its working tree or repository state moves.
    GitStatus,
}

/// What limits a project's watching for one purpose: an outright refusal, or a degradation to a
/// reduced set that still covers what matters most.
///
/// A refusal means nothing is watched for that purpose — the OS turned the whole registration
/// down. A degradation means the purpose's essential watches (a project's repository state, its
/// root, the directories a `restart_when_changed` glob names explicitly) are held, but its
/// tree needs more watches than its share of the system's budget, so the speculative whole-tree
/// scan was dropped rather than registered incomplete.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchLimit {
    /// Nothing is watched for this purpose — the OS turned the registration down outright.
    Refused(WatchError),
    /// The essential watches are held; the speculative whole-tree scan was not, for want of
    /// budget.
    Degraded,
}

/// What one purpose's re-sync met for one project: the limit it was given, or `None` for a watch
/// it holds without restriction.
///
/// Reported for every project that purpose watches, granted or limited, so a limit standing for a
/// project it has since stopped watching can be withdrawn rather than left in place.
pub struct WatchOutcome {
    pub project: ProjectId,
    pub limit: Option<WatchLimit>,
}
