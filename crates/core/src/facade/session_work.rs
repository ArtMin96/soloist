//! The session-work read model (context C8 → C6): what one process holds a lock on now, or
//! touched through a tool this run — the agent terminal header's "current work" / "this session"
//! context, plus the recording helpers the sibling todo, scratchpad, and link surfaces call
//! through after a successful, session-scoped access.
//!
//! "Current work" (a held todo lock) is derived on read from [`Todos::list`](crate::coordination::Todos::list);
//! "this session" (what was read or written through a tool) comes from the
//! [`SessionActivity`](crate::coordination::SessionActivity) registry. A recorded id with no live
//! document is dropped on the join, so a deleted todo or scratchpad can never strand a stale title
//! in the header.

use super::scoped::ScopedFacade;
use super::Facade;
use crate::coordination::AccessKind;
use crate::ids::{ProcessId, ScratchpadId, TodoId};
use crate::orchestration::{SessionScratchpad, SessionTodo, SessionWork};
use crate::ports::StoreError;

impl Facade {
    /// The coordination documents `process` holds now or touched this run. `None` when the
    /// process has nothing recorded in the session-activity registry — an untouched process, or
    /// one whose run has ended (the registry forgets a process the moment it reaches a terminal
    /// state). A local read like [`process_view`](Self::process_view): authorization is the
    /// caller's.
    pub fn session_work(&self, process: ProcessId) -> Result<Option<SessionWork>, StoreError> {
        let recorded_todos = self.session_activity.todos(process);
        let recorded_scratchpads = self.session_activity.scratchpads(process);
        if recorded_todos.is_empty() && recorded_scratchpads.is_empty() {
            return Ok(None);
        }
        // A recorded process with no live view has already closed; its record is about to be (or
        // just was) forgotten by the same close hook, so there is no project to report it under.
        let Some(project) = self.process_view(process).map(|view| view.project) else {
            return Ok(None);
        };

        let todo_summaries = self.todos.list(project)?;
        let scratchpad_summaries = self.scratchpads.list(project)?;

        let mut todos = Vec::with_capacity(recorded_todos.len());
        let mut locked: Vec<_> = todo_summaries
            .iter()
            .filter(|summary| summary.locked_by == Some(process))
            .collect();
        locked.sort_by_key(|summary| summary.id);
        for summary in locked {
            let access = access_of(&recorded_todos, summary.id);
            todos.push(SessionTodo {
                id: summary.id,
                title: summary.title.clone(),
                status: summary.status,
                blocked: summary.blocked,
                locked: true,
                access,
            });
        }
        for (id, kind) in &recorded_todos {
            if todos.iter().any(|todo| todo.id == *id) {
                continue;
            }
            if let Some(summary) = todo_summaries.iter().find(|summary| summary.id == *id) {
                todos.push(SessionTodo {
                    id: summary.id,
                    title: summary.title.clone(),
                    status: summary.status,
                    blocked: summary.blocked,
                    locked: false,
                    access: Some(*kind),
                });
            }
        }

        let scratchpads = recorded_scratchpads
            .iter()
            .filter_map(|(id, kind)| {
                scratchpad_summaries
                    .iter()
                    .find(|summary| summary.id == *id)
                    .map(|summary| SessionScratchpad {
                        id: summary.id,
                        name: summary.name.clone(),
                        access: *kind,
                    })
            })
            .collect();

        Ok(Some(SessionWork {
            process,
            project,
            todos,
            scratchpads,
        }))
    }
}

/// The recorded access kind for `id` in `recorded`, or `None` when it was never recorded — the
/// state of a lock this process holds without having read or written it through a tool this run.
fn access_of(recorded: &[(TodoId, AccessKind)], id: TodoId) -> Option<AccessKind> {
    recorded
        .iter()
        .find(|(recorded_id, _)| *recorded_id == id)
        .map(|(_, kind)| *kind)
}

impl ScopedFacade<'_> {
    /// The process this session is bound to, or `None` for an external or unbound caller. Recording
    /// an access is attributed to a genuine Soloist-supervised process only, never to an
    /// externally-registered label: unlike [`coordination_owner`](Facade::coordination_owner),
    /// finding none here is not a refusal — it just means this caller's access is not recorded.
    pub(in crate::facade) fn acting_process(&self) -> Option<ProcessId> {
        self.inner.identity.origin(self.session).process()
    }

    /// Records that this session's bound process touched todo `id` at `kind`. A no-op for a caller
    /// with no bound process.
    pub(in crate::facade) fn note_todo(&self, id: TodoId, kind: AccessKind) {
        if let Some(process) = self.acting_process() {
            self.inner.session_activity.record_todo(process, id, kind);
        }
    }

    /// Records that this session's bound process touched scratchpad `id` at `kind`. A no-op for a
    /// caller with no bound process.
    pub(in crate::facade) fn note_scratchpad(&self, id: ScratchpadId, kind: AccessKind) {
        if let Some(process) = self.acting_process() {
            self.inner
                .session_activity
                .record_scratchpad(process, id, kind);
        }
    }
}

#[cfg(test)]
#[path = "session_work_tests.rs"]
mod tests;
