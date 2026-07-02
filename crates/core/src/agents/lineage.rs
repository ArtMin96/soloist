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
//! manual launch records nothing and so reads back as a root. The map is the parent *hint*; the
//! live registry stays the source of truth for who exists, so a node whose parent has left the
//! registry reads back as a root with no explicit re-parenting.

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

    /// The agent that spawned `child`, or `None` if it has no recorded parent (a manual or
    /// unbound launch). The caller still confirms the parent is live before treating it as an
    /// edge — a node whose parent has left the registry is a root.
    pub fn parent_of(&self, child: ProcessId) -> Option<ProcessId> {
        lock(&self.parents).get(&child).copied()
    }

    /// Every recorded `(child, parent)` pair, sorted by child id for a stable read. The caller
    /// still filters both ends against the live registry — an edge whose parent has left the
    /// registry is not a tree edge, exactly as with [`Self::parent_of`].
    pub fn edges(&self) -> Vec<(ProcessId, ProcessId)> {
        let mut edges: Vec<_> = lock(&self.parents)
            .iter()
            .map(|(child, parent)| (*child, *parent))
            .collect();
        edges.sort_by_key(|(child, _)| *child);
        edges
    }

    /// Drops lineage for any worker no longer in `live` (gone from the registry), so the map
    /// never outgrows the live process set.
    pub fn retain_live(&self, live: &HashSet<ProcessId>) {
        lock(&self.parents).retain(|child, _| live.contains(child));
    }
}

#[cfg(test)]
#[path = "lineage_tests.rs"]
mod tests;
