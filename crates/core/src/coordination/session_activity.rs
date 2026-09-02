//! The per-run session-activity record (context C6): which coordination documents a bound agent
//! read or wrote through a tool call this run — the agent terminal header's "current work" / "this
//! session" context.
//!
//! Distinct from a todo's durable lock and from a scratchpad's revision: this is ephemeral,
//! in-memory bookkeeping of *access*, not ownership or content. It exists purely to answer "what has
//! this process touched", the way [`AttentionRegistry`](crate::notify::AttentionRegistry) answers
//! "what does this process have waiting" — the same shape, for a different question.
//!
//! Bounded per process by [`MAX_SESSION_DOCUMENTS_PER_PROCESS`]; bounded in aggregate by the live
//! process registry, since the supervisor's close hook drops a process's whole entry the moment it
//! reaches a terminal state (see the [`LockReleaser`] impl below), so nothing here can outlive the
//! process it was recorded against.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ScratchpadId, TodoId};
use crate::ports::LockReleaser;
use crate::sync::lock;

/// How a bound process touched a coordination document this run: read through a tool, or written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessKind {
    Loaded,
    Worked,
}

/// The most todos — and, separately, the most scratchpads — recorded per process in one run. A
/// long-lived agent that reads and writes without stopping does not grow this without bound; past
/// the cap the oldest access is evicted to make room for the newest, which is what a live "current
/// work" header needs more than a complete history.
pub const MAX_SESSION_DOCUMENTS_PER_PROCESS: usize = 64;

/// One process's recorded accesses this run: two independently insertion-ordered, independently
/// capped lists, so a flood of scratchpad reads can never evict a todo access or vice versa.
#[derive(Default)]
struct Touched {
    todos: Vec<(TodoId, AccessKind)>,
    scratchpads: Vec<(ScratchpadId, AccessKind)>,
}

/// Records `id` against `entries` at `kind`: an existing entry is updated **in place**, without
/// reordering, so a document already touched keeps the position it was first touched at; a `Loaded`
/// never overwrites a recorded `Worked`, since a write already told the fuller story of what
/// happened this run. A fresh entry past the cap evicts the oldest (index `0`) to make room.
/// Returns whether the access actually changed anything, so the caller announces only a real
/// change.
fn record<Id: PartialEq>(entries: &mut Vec<(Id, AccessKind)>, id: Id, kind: AccessKind) -> bool {
    if let Some((_, existing)) = entries.iter_mut().find(|(existing, _)| *existing == id) {
        if *existing == AccessKind::Loaded && kind == AccessKind::Worked {
            *existing = AccessKind::Worked;
            true
        } else {
            false
        }
    } else {
        if entries.len() >= MAX_SESSION_DOCUMENTS_PER_PROCESS {
            entries.remove(0);
        }
        entries.push((id, kind));
        true
    }
}

/// The per-run session-activity registry: what each bound process has read or written through a
/// tool call so far, keyed by process. Shared behind an `Arc` between the façade methods that
/// record an access and query it, and the supervisor's close hook that clears a process's entry.
pub struct SessionActivity {
    bus: EventBus,
    touched: Mutex<HashMap<ProcessId, Touched>>,
}

impl SessionActivity {
    /// A registry with nothing recorded, announcing every real change on `bus`.
    pub fn new(bus: EventBus) -> Self {
        Self {
            bus,
            touched: Mutex::new(HashMap::new()),
        }
    }

    /// Records that `process` touched `todo` at `kind`, publishing
    /// [`DomainEvent::SessionWorkChanged`] only when the record actually changed (a fresh access, or
    /// an upgrade from `Loaded` to `Worked`) — a repeat of the same access wakes no subscriber to
    /// re-read an unchanged header.
    pub fn record_todo(&self, process: ProcessId, todo: TodoId, kind: AccessKind) {
        let changed = record(
            &mut lock(&self.touched).entry(process).or_default().todos,
            todo,
            kind,
        );
        if changed {
            self.bus
                .publish(DomainEvent::SessionWorkChanged { process });
        }
    }

    /// Records that `process` touched `scratchpad` at `kind` (see
    /// [`record_todo`](Self::record_todo) for the change/no-change and upgrade rules).
    pub fn record_scratchpad(
        &self,
        process: ProcessId,
        scratchpad: ScratchpadId,
        kind: AccessKind,
    ) {
        let changed = record(
            &mut lock(&self.touched).entry(process).or_default().scratchpads,
            scratchpad,
            kind,
        );
        if changed {
            self.bus
                .publish(DomainEvent::SessionWorkChanged { process });
        }
    }

    /// The todos `process` has touched this run, oldest first, or empty if it has none recorded.
    pub fn todos(&self, process: ProcessId) -> Vec<(TodoId, AccessKind)> {
        lock(&self.touched)
            .get(&process)
            .map(|touched| touched.todos.clone())
            .unwrap_or_default()
    }

    /// The scratchpads `process` has touched this run, oldest first, or empty if it has none
    /// recorded.
    pub fn scratchpads(&self, process: ProcessId) -> Vec<(ScratchpadId, AccessKind)> {
        lock(&self.touched)
            .get(&process)
            .map(|touched| touched.scratchpads.clone())
            .unwrap_or_default()
    }

    /// Drops everything recorded for `process` — the per-run record ends when the process does.
    pub fn forget(&self, process: ProcessId) {
        lock(&self.touched).remove(&process);
    }
}

impl LockReleaser for SessionActivity {
    fn release_all(&self, process: ProcessId) {
        self.forget(process);
    }
}

#[cfg(test)]
#[path = "session_activity_tests.rs"]
mod tests;
