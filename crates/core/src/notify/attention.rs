//! The unread registry: which processes have raised an alert the user has not dealt with yet.
//!
//! One source for every surface that shows unread — the process row's marker, the project header's
//! dot, the title-bar count, and the app-icon badge. A second copy of this in a frontend store
//! would have to be kept in step with the badge adapter's, so there is exactly one, and each
//! surface derives what it draws from [`AttentionRegistry::snapshot`].
//!
//! Entries live only as long as the process does: the reactor drops one when its process leaves
//! the registry, so a count can never strand on something that no longer exists.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;

use crate::attention::AttentionKind;
use crate::ids::ProcessId;
use crate::sync::lock;

/// What one process has waiting for the user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProcessAttention {
    pub process: ProcessId,
    /// The kinds raised since this process was last cleared, oldest first — a surface renders the
    /// marker from the most severe or the most recent as it chooses.
    pub kinds: Vec<AttentionKind>,
}

/// Everything unread, derived on read. Never stored: a cached second copy would be one more thing
/// to invalidate when a process goes away.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AttentionSnapshot {
    /// Every process with something waiting, ordered by process id so a surface renders a stable
    /// list without sorting it again.
    pub processes: Vec<ProcessAttention>,
    /// How many alerts are waiting in total. Counts alerts rather than processes, and is never
    /// truncated — a display cap such as "99+" belongs to whatever renders it.
    pub total: usize,
}

/// The processes with unread attention. Shared behind an `Arc` between the reactor that raises and
/// the façade methods that clear.
#[derive(Debug, Default)]
pub struct AttentionRegistry {
    unread: Mutex<HashMap<ProcessId, Vec<AttentionKind>>>,
}

impl AttentionRegistry {
    /// A registry with nothing unread.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `process` raised `kind`. Repeats accumulate: two crashes are two things that
    /// happened, and collapsing them would under-report a process that keeps failing.
    pub fn raise(&self, process: ProcessId, kind: AttentionKind) {
        lock(&self.unread).entry(process).or_default().push(kind);
    }

    /// Forgets what `process` had waiting, reporting whether anything was actually there. The
    /// callers announce a change on the bus, so a clear that removed nothing must say so rather
    /// than waking every surface to re-read an unchanged snapshot.
    pub fn clear(&self, process: ProcessId) -> bool {
        lock(&self.unread).remove(&process).is_some()
    }

    /// Forgets everything, reporting whether anything was there.
    pub fn clear_all(&self) -> bool {
        let mut unread = lock(&self.unread);
        let had_any = !unread.is_empty();
        unread.clear();
        had_any
    }

    /// Everything currently unread.
    pub fn snapshot(&self) -> AttentionSnapshot {
        let unread = lock(&self.unread);
        let mut processes: Vec<ProcessAttention> = unread
            .iter()
            .map(|(process, kinds)| ProcessAttention {
                process: *process,
                kinds: kinds.clone(),
            })
            .collect();
        processes.sort_by_key(|entry| entry.process);
        AttentionSnapshot {
            total: processes.iter().map(|entry| entry.kinds.len()).sum(),
            processes,
        }
    }
}

#[cfg(test)]
#[path = "attention_tests.rs"]
mod tests;
