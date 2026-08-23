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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProcessAttention {
    pub process: ProcessId,
    /// The kind that started the run still waiting. A surface draws its marker from this one, so a
    /// process reads as what first asked for the user rather than as whatever it last raised.
    pub kind: AttentionKind,
    /// How many alerts are waiting on this process, up to [`MAX_UNREAD_PER_PROCESS`].
    pub alerts: u32,
}

/// Everything unread, derived on read. Never stored: a cached second copy would be one more thing
/// to invalidate when a process goes away.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AttentionSnapshot {
    /// Every process with something waiting, ordered by process id so a surface renders a stable
    /// list without sorting it again.
    pub processes: Vec<ProcessAttention>,
    /// How many alerts are waiting in total. Counts alerts rather than processes, and carries no
    /// display cap — a reading such as "99+" belongs to whatever renders it, and the only ceiling
    /// here is [`MAX_UNREAD_PER_PROCESS`], far past what any surface prints in full.
    pub total: usize,
}

/// The most alerts one process is counted for. A terminal rings the bell once per BEL byte, so
/// this count is fed by whatever a child process chooses to print; past this many, a larger number
/// tells a user nothing they cannot already act on. It sits two orders of magnitude above any
/// display cap, so what a surface prints is never this bound showing through.
const MAX_UNREAD_PER_PROCESS: u32 = 9_999;

/// What one process has waiting, as the registry keeps it: the kind that started the run and how
/// many have landed since. One fixed-size record per process, so what a process can raise decides
/// the count it reports and never the memory it occupies.
#[derive(Clone, Copy, Debug)]
struct Unread {
    first: AttentionKind,
    count: u32,
}

/// The processes with unread attention. Shared behind an `Arc` between the reactor that raises and
/// the façade methods that clear.
#[derive(Debug, Default)]
pub struct AttentionRegistry {
    unread: Mutex<HashMap<ProcessId, Unread>>,
}

impl AttentionRegistry {
    /// A registry with nothing unread.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `process` raised `kind`. Repeats accumulate: two crashes are two things that
    /// happened, and collapsing them would under-report a process that keeps failing. The count
    /// stops at [`MAX_UNREAD_PER_PROCESS`], so a process that alerts without stopping raises the
    /// number it reports and nothing else.
    pub fn raise(&self, process: ProcessId, kind: AttentionKind) {
        lock(&self.unread)
            .entry(process)
            .and_modify(|unread| {
                unread.count = unread.count.saturating_add(1).min(MAX_UNREAD_PER_PROCESS);
            })
            .or_insert(Unread {
                first: kind,
                count: 1,
            });
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
        let mut processes: Vec<ProcessAttention> = lock(&self.unread)
            .iter()
            .map(|(process, unread)| ProcessAttention {
                process: *process,
                kind: unread.first,
                alerts: unread.count,
            })
            .collect();
        processes.sort_by_key(|entry| entry.process);
        AttentionSnapshot {
            total: processes.iter().map(|entry| entry.alerts as usize).sum(),
            processes,
        }
    }
}

#[cfg(test)]
#[path = "attention_tests.rs"]
mod tests;
