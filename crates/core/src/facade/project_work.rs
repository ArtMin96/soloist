//! The project-work read model (context C8 → C2/C6): the coordination documents a project's live
//! processes hold or touched this run — the sidebar's Todos and Scratchpads groups — plus the
//! recording helpers the sibling todo, scratchpad, and link surfaces call through after a
//! successful, session-scoped access.
//!
//! Every participant's role is derived on read from [`Todos::list`](crate::coordination::Todos::list)
//! and the [`SessionActivity`](crate::coordination::SessionActivity) registry, joined against the
//! live process registry so a process that has closed never strands a row. A recorded id with no
//! live document is dropped on the join, so a deleted todo or scratchpad can never strand a stale
//! title in the sidebar.

use super::scoped::ScopedFacade;
use super::Facade;
use crate::coordination::{AccessKind, TodoStatus};
use crate::ids::{ProcessId, ProjectId, ScratchpadId, TodoId};
use crate::orchestration::{
    DocumentParticipant, DocumentRole, ProjectWork, ScratchpadWork, TodoWork,
};
use crate::ports::StoreError;
use crate::process::ProcessView;

/// One live process's captured session activity, read once per process rather than once per
/// (document, process) pair.
struct LiveProcess {
    id: ProcessId,
    label: String,
    todos: Vec<(TodoId, AccessKind)>,
    scratchpads: Vec<(ScratchpadId, AccessKind)>,
}

impl Facade {
    /// The coordination documents `project`'s live processes hold or touched this run, grouped by
    /// document rather than by process. A project with nothing touched returns empty lists — this
    /// query returns [`ProjectWork`], never `Option`. A local read like
    /// [`process_view`](Self::process_view): authorization is the caller's.
    pub fn project_work(&self, project: ProjectId) -> Result<ProjectWork, StoreError> {
        let mut live: Vec<LiveProcess> = self
            .snapshot()
            .into_iter()
            .filter(|view: &ProcessView| view.project == project)
            .map(|view| LiveProcess {
                id: view.id,
                label: view.label,
                todos: self.session_activity.todos(view.id),
                scratchpads: self.session_activity.scratchpads(view.id),
            })
            .collect();
        live.sort_by_key(|process| process.id);

        let todo_summaries = self.todos.list(project)?;
        let todos = todo_summaries
            .into_iter()
            .filter(|summary| summary.status != TodoStatus::Done)
            .filter_map(|summary| {
                let participants: Vec<_> = live
                    .iter()
                    .filter_map(|process| {
                        let role = if summary.locked_by == Some(process.id) {
                            DocumentRole::Implementing
                        } else {
                            match access_of(&process.todos, summary.id)? {
                                AccessKind::Worked => DocumentRole::Editing,
                                AccessKind::Loaded => DocumentRole::Reading,
                            }
                        };
                        Some(DocumentParticipant {
                            process: process.id,
                            label: process.label.clone(),
                            role,
                        })
                    })
                    .collect();
                if participants.is_empty() {
                    return None;
                }
                Some(TodoWork {
                    id: summary.id,
                    title: summary.title,
                    status: summary.status,
                    participants,
                })
            })
            .collect();

        let scratchpad_summaries = self.scratchpads.list(project)?;
        let scratchpads = scratchpad_summaries
            .into_iter()
            .filter_map(|summary| {
                let participants: Vec<_> = live
                    .iter()
                    .filter_map(|process| {
                        let role = match access_of(&process.scratchpads, summary.id)? {
                            AccessKind::Worked => DocumentRole::Editing,
                            AccessKind::Loaded => DocumentRole::Reading,
                        };
                        Some(DocumentParticipant {
                            process: process.id,
                            label: process.label.clone(),
                            role,
                        })
                    })
                    .collect();
                if participants.is_empty() {
                    return None;
                }
                Some(ScratchpadWork {
                    id: summary.id,
                    name: summary.name,
                    participants,
                })
            })
            .collect();

        Ok(ProjectWork {
            project,
            todos,
            scratchpads,
        })
    }
}

/// The recorded access kind for `id` in `recorded`, or `None` when it was never recorded.
fn access_of<Id: PartialEq>(recorded: &[(Id, AccessKind)], id: Id) -> Option<AccessKind> {
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
#[path = "project_work_tests.rs"]
mod tests;
