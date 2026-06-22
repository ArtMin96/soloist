//! The registry of agents under idle classification.
//!
//! This is where the agent context (C4) holds each launched agent's provider — the one fact
//! the supervisor (C2) deliberately does not carry, so its process model stays free of the
//! agent taxonomy. The launch path registers an agent here; the [`IdleSampler`](super::sampler)
//! reads and advances it each sample. Shared behind an `Arc` between the two.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use crate::agents::AgentKind;
use crate::ids::ProcessId;
use crate::sync::lock;
use crate::terminal::TerminalActivity;

use super::activity::AgentActivity;
use super::classifier::Classifier;

/// Tracks the activity classifier of every launched agent, keyed by process. Cloneable state
/// is unnecessary — it is shared behind an `Arc`; the launch path calls [`Self::track`] and
/// the sampler drives the rest.
#[derive(Default)]
pub struct IdleTracker {
    agents: Mutex<HashMap<ProcessId, Classifier>>,
}

impl IdleTracker {
    /// An empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Begins classifying a launched agent under its provider's heuristic. Called once when
    /// an agent is launched; a re-track resets it.
    pub fn track(&self, id: ProcessId, kind: AgentKind) {
        lock(&self.agents).insert(id, Classifier::new(kind));
    }

    /// The process ids currently tracked.
    pub(super) fn tracked(&self) -> Vec<ProcessId> {
        lock(&self.agents).keys().copied().collect()
    }

    /// Feeds a running agent its latest terminal signals; returns the new activity if it
    /// changed (the edge to emit). A no-op (returns `None`) for an untracked id.
    pub(super) fn observe(&self, id: ProcessId, signals: &TerminalActivity) -> Option<AgentActivity> {
        lock(&self.agents)
            .get_mut(&id)
            .and_then(|classifier| classifier.observe(signals))
    }

    /// Resets a tracked agent that is not currently running, so a relaunch re-emits its
    /// first activity.
    pub(super) fn reset(&self, id: ProcessId) {
        if let Some(classifier) = lock(&self.agents).get_mut(&id) {
            classifier.reset();
        }
    }

    /// Drops tracking for any agent no longer in `live` (gone from the registry), so the map
    /// never outgrows the live process set.
    pub(super) fn retain_live(&self, live: &HashSet<ProcessId>) {
        lock(&self.agents).retain(|id, _| live.contains(id));
    }
}

#[cfg(test)]
#[path = "tracker_tests.rs"]
mod tests;
