//! The orchestration read-model query (context C8 → C2/C4/C6): the one read the orchestration UI
//! renders.
//!
//! [`orchestration_snapshot`](Facade::orchestration_snapshot) composes a project's view across three
//! contexts — the process registry (C2), the idle tracker (C4), and the coordination aggregates
//! (C6) — into one [`OrchestrationSnapshot`]. It is the query half of CQRS-lite: every field is read
//! from the authoritative source and **derived on read**, never cached, so the projection cannot
//! drift from the state it mirrors. A live UI pairs this with the coordination
//! [`DomainEvent`](crate::events::DomainEvent)s, re-reading the snapshot when one signals a change.

use super::Facade;
use crate::ids::ProjectId;
use crate::orchestration::{AgentNode, OrchestrationSnapshot};
use crate::ports::StoreError;

impl Facade {
    /// The orchestration read-model for `project`: its agent lineage tree (each managed process with
    /// its supervision status and, for agents, live idle activity) plus the coordination state agents
    /// share — todos, timers, leases, scratchpads, and key-value. Assembled purely from existing
    /// reads; it starts no work and mutates nothing.
    ///
    /// **Authorization is the caller's.** Like [`snapshot`](Self::snapshot) this is a local read: it
    /// filters by the `project` it is handed and trusts the caller to be entitled to it — sound for the
    /// local UI, which already has full access to its own projects. It must therefore never take a
    /// `project` straight from an untrusted surface: an adapter exposing it over MCP or HTTP has to
    /// derive `project` from the caller's bound, identity-checked scope, never a value the caller chose.
    pub fn orchestration_snapshot(
        &self,
        project: ProjectId,
    ) -> Result<OrchestrationSnapshot, StoreError> {
        let agents = self
            .snapshot()
            .into_iter()
            .filter(|view| view.project == project)
            .map(|view| AgentNode {
                id: view.id,
                // Spawn lineage is not recorded yet; until then every node is a root.
                parent: None,
                kind: view.kind,
                status: view.status,
                activity: self.idle.activity(view.id),
            })
            .collect();
        Ok(OrchestrationSnapshot {
            project,
            agents,
            todos: self.todos.views(project)?,
            timers: self.timers.list_project(project)?,
            leases: self.leases.list(project)?,
            scratchpads: self.scratchpads.list(project)?,
            kv: self.kv.list(project)?,
        })
    }
}

#[cfg(test)]
#[path = "orchestration_tests.rs"]
mod tests;
