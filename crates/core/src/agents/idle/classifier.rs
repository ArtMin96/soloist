//! Per-agent activity classification: wraps a provider's [`IdleStrategy`] with the rolling
//! memory it needs and turns the stream of samples into edge-triggered transitions — it
//! reports an activity only when it *changes*, so adapters update a row without polling.

use crate::agents::AgentKind;
use crate::terminal::TerminalActivity;

use super::activity::AgentActivity;
use super::strategy::{strategy_for, AgentMemory, IdleStrategy};

/// Tracks one agent's activity over successive terminal samples. Holds the provider's
/// heuristic, its rolling memory, and the last activity reported, so it can emit only on a
/// transition.
pub(super) struct Classifier {
    strategy: &'static dyn IdleStrategy,
    memory: AgentMemory,
    current: Option<AgentActivity>,
}

impl Classifier {
    /// A classifier for an agent of the given provider, before its first sample.
    pub(super) fn new(kind: AgentKind) -> Self {
        Self {
            strategy: strategy_for(kind),
            memory: AgentMemory::default(),
            current: None,
        }
    }

    /// Feeds one terminal sample. Returns `Some(activity)` when the classification changed
    /// (the edge to emit) — including the very first sample — and `None` when it held steady.
    pub(super) fn observe(&mut self, signals: &TerminalActivity) -> Option<AgentActivity> {
        // Before the first sample treat the agent as idle, so a brief pause settles to the
        // previous state rather than flapping (see the strategies' `current` handling).
        let previous = self.current.unwrap_or(AgentActivity::Idle);
        let next = self.strategy.classify(&mut self.memory, signals, previous);
        if self.current == Some(next) {
            None
        } else {
            self.current = Some(next);
            Some(next)
        }
    }

    /// The activity last reported for this agent, or `None` before its first sample. A snapshot
    /// read (not edge-triggered) so a caller can ask "is this agent idle right now?" — used to
    /// report whether a fire-when-idle timer's condition is already met at set time.
    pub(super) fn current(&self) -> Option<AgentActivity> {
        self.current
    }

    /// Resets to the pre-sample state, so an agent that stopped and is relaunched re-emits
    /// its first activity. Called while the agent is not running.
    pub(super) fn reset(&mut self) {
        self.memory = AgentMemory::default();
        self.current = None;
    }
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
