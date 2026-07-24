//! The orchestration read-model query (context C8 → C2/C4/C6): the one read the orchestration UI
//! renders.
//!
//! [`orchestration_snapshot`](Facade::orchestration_snapshot) composes a project's view across three
//! contexts — the process registry (C2), the idle tracker (C4), and the coordination aggregates
//! (C6) — into one [`OrchestrationSnapshot`]. It is the query half of CQRS-lite: every field is read
//! from the authoritative source and **derived on read**, never cached, so the projection cannot
//! drift from the state it mirrors. A live UI pairs this with the coordination
//! [`DomainEvent`](crate::events::DomainEvent)s, re-reading the snapshot when one signals a change.

use std::collections::HashSet;

use super::Facade;
use crate::coordination::watched_is_idle;
use crate::ids::{ProcessId, ProjectId};
use crate::orchestration::{AgentNode, AgentSignal, LineageEdge, OrchestrationSnapshot};
use crate::ports::StoreError;

impl Facade {
    /// Every tracked agent's current idle activity across all projects — the snapshot the UI's
    /// signal store seeds from so a dropped
    /// [`AgentActivityChanged`](crate::events::DomainEvent::AgentActivityChanged) during bus lag,
    /// or a webview reload, recovers the idle badges rather than running on edge-triggered stale
    /// state. Derived on read from the idle tracker (C4); only agents classified at least once
    /// appear. A local read like [`snapshot`](Self::snapshot): authorization is the caller's, so
    /// an adapter exposing it over MCP/HTTP would have to scope it to the caller's own project.
    pub fn agent_activity(&self) -> Vec<AgentSignal> {
        self.idle
            .activity_snapshot()
            .into_iter()
            .map(|(id, activity)| AgentSignal { id, activity })
            .collect()
    }

    /// Every live spawn-lineage edge across all projects — the sidebar's cross-project nesting
    /// read, cheap enough to re-query on process lifecycle events. An edge appears only while
    /// both its worker and its lead are in the registry, the same re-root-on-read rule as
    /// [`orchestration_snapshot`](Self::orchestration_snapshot); each edge is intra-project by
    /// construction (a worker always lands in its caller's own project). A local read like
    /// [`snapshot`](Self::snapshot): authorization is the caller's.
    pub fn lineage_edges(&self) -> Vec<LineageEdge> {
        let present: HashSet<ProcessId> = self.snapshot().into_iter().map(|view| view.id).collect();
        self.lineage
            .edges()
            .into_iter()
            .filter(|(child, parent)| present.contains(child) && present.contains(parent))
            .map(|(child, parent)| LineageEdge { child, parent })
            .collect()
    }

    /// The orchestration read-model for `project`: its agent lineage tree (each managed process with
    /// its supervision status and, for agents, live idle activity) plus the coordination state agents
    /// share — todos, timers, leases, scratchpads, diagrams, and key-value. Assembled purely from existing
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
        let views: Vec<_> = self
            .snapshot()
            .into_iter()
            .filter(|view| view.project == project)
            .collect();
        let present: HashSet<ProcessId> = views.iter().map(|view| view.id).collect();
        let agents = views
            .into_iter()
            .map(|view| AgentNode {
                id: view.id,
                // A recorded parent is an edge only while that lead is still present: a lead
                // that has left the registry re-roots its workers, so a closed lead never
                // strands a node (the registry is the source of truth for who exists).
                parent: self
                    .lineage
                    .parent_of(view.id)
                    .filter(|parent| present.contains(parent)),
                label: view.label,
                kind: view.kind,
                status: view.status,
                activity: self.idle.activity(view.id),
            })
            .collect();
        // Enrich each timer view with `waiting_on` (watched but not yet idle) and `already_idle`
        // (quorum met at read time) computed from the live idle tracker and process registry. These
        // are dynamic at-read-time values, not stored — the aggregate defaults them to empty/false.
        let timers = self
            .timers
            .list_project(project)?
            .into_iter()
            .map(|mut tv| {
                let enrichment = tv.fire.idle_quorum().map(|(mode, watched)| {
                    let is_idle = |p: ProcessId| {
                        watched_is_idle(self.idle.activity(p), self.supervisor.view(p).is_some())
                    };
                    let waiting_on: Vec<ProcessId> =
                        watched.iter().copied().filter(|&p| !is_idle(p)).collect();
                    let already_idle = mode.quorum_met(watched, is_idle);
                    (waiting_on, already_idle)
                });
                if let Some((waiting_on, already_idle)) = enrichment {
                    tv.waiting_on = waiting_on;
                    tv.already_idle = already_idle;
                }
                tv
            })
            .collect();
        Ok(OrchestrationSnapshot {
            project,
            agents,
            todos: self.todos.views(project)?,
            timers,
            leases: self.leases.list(project)?,
            scratchpads: self.scratchpads.list(project)?,
            diagrams: self.diagrams.list(project)?,
            kv: self.kv.list(project)?,
        })
    }
}

#[cfg(test)]
#[path = "orchestration_tests.rs"]
mod tests;
