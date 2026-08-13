//! Agent spawn lineage (part of context C4): which agent spawned which.
//!
//! When a lead agent spawns a worker, the worker's parent is the lead — the bound session
//! owner known at spawn time. This is the one fact the supervisor (C2) deliberately does not
//! carry, so its process model stays free of the agent taxonomy, mirroring how the
//! [`IdleTracker`](super::idle::IdleTracker) holds each agent's provider. The orchestration
//! read-model joins it to render the lead→worker tree.
//!
//! Lineage is **per-run, in-memory** process metadata, never persisted: a parent id is only
//! meaningful while that process is live, so it is reconstructed from spawns, not restored. A
//! manual launch records nothing and so reads back as a root. Edges are retained for the run so a
//! departed lead does not merge its descendants into unrelated orchestration groups.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use crate::ids::ProcessId;
use crate::sync::lock;

/// Tracks each spawned worker's parent agent, keyed by the worker process. Shared behind an
/// `Arc`: the spawn path calls [`Self::record`], the orchestration read-model calls
/// [`Self::parent_of`], and the idle sampler prunes departed entries via [`Self::retain_live`].
#[derive(Default)]
pub struct AgentLineage {
    parents: Mutex<HashMap<ProcessId, ProcessId>>,
}

impl AgentLineage {
    /// An empty lineage tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that `child` was spawned by `parent`. Called once when a bound lead spawns a
    /// worker; a manual or unbound launch records nothing, leaving the child a root.
    pub fn record(&self, child: ProcessId, parent: ProcessId) {
        lock(&self.parents).insert(child, parent);
    }

    /// The agent that spawned `child`, or `None` if it has no retained parent. A dead parent can
    /// remain here while it connects a live descendant to its authorization root.
    pub fn parent_of(&self, child: ProcessId) -> Option<ProcessId> {
        lock(&self.parents).get(&child).copied()
    }

    /// The oldest recorded ancestor of `process`, following retained edges and stopping safely if
    /// corrupt input ever forms a cycle.
    pub fn root_of(&self, process: ProcessId) -> ProcessId {
        let parents = lock(&self.parents);
        let mut root = process;
        let mut visited = HashSet::new();
        while visited.insert(root) {
            let Some(parent) = parents.get(&root) else {
                break;
            };
            root = *parent;
        }
        root
    }

    /// Every retained `(child, parent)` pair, sorted by child id for a stable read.
    pub fn edges(&self) -> Vec<(ProcessId, ProcessId)> {
        let mut edges: Vec<_> = lock(&self.parents)
            .iter()
            .map(|(child, parent)| (*child, *parent))
            .collect();
        edges.sort_by_key(|(child, _)| *child);
        edges
    }

    /// Keeps live agents and the dead ancestor edges needed to connect them, dropping every dead
    /// leaf and every lineage group with no live member.
    pub fn retain_live(&self, live: &HashSet<ProcessId>) {
        let mut parents = lock(&self.parents);
        let mut needed = live.clone();
        let mut frontier: Vec<_> = live.iter().copied().collect();
        while let Some(process) = frontier.pop() {
            if let Some(parent) = parents.get(&process).copied() {
                if needed.insert(parent) {
                    frontier.push(parent);
                }
            }
        }
        parents.retain(|child, _| needed.contains(child));
    }
}

#[cfg(test)]
#[path = "lineage_tests.rs"]
mod tests;
