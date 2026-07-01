//! The summary reactor: when an agent goes idle, produce a one-line summary of its recent output.
//!
//! Optional and degradable. It subscribes to the event bus and, on an agent's transition to
//! [`AgentActivity::Idle`], reads the summarization opt-in: with a summarizer tool configured, it
//! sends a compact rendered-text snapshot to that tool run headless (the per-provider
//! [`strategy`](super::strategy), executed off the runtime through the [`SummaryRunner`] port) and
//! publishes a [`DomainEvent::AgentSummary`]. With summarization off (the default), no runner
//! wired, an unsupported provider, or any failure, it produces nothing and never blocks the core —
//! idle detection stays heuristic-only. A per-agent cooldown rate-limits re-summaries.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::agents::{AgentActivity, AgentTool, AgentToolRepo};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::settings::{AgentSettings, Settings, SettingsRepo};
use crate::supervision::run_blocking;

use super::prompt::{build_prompt, SNAPSHOT_LINES};
use super::runner::{SummaryInvocation, SummaryRunner};
use super::snapshot::OutputSnapshot;
use super::strategy::summary_strategy_for;

/// The shortest gap between summaries for one agent, so rapid working/idle flapping — or a failing
/// summarizer — can't spawn a summary every idle tick. An attempt (success or failure) starts the
/// cooldown, so a broken CLI is not re-run each transition. Within the 15s–1min cadence Solo uses.
const COOLDOWN: Duration = Duration::from_secs(30);

/// The maximum length of a published summary in characters, so a verbose model reply can't carry
/// an unbounded string onto the event bus and into every adapter's read model.
const MAX_SUMMARY_CHARS: usize = 200;

/// Turns the idle transitions of agents into one-line summaries. Built once by the composition
/// root (via [`crate::facade::Facade::summary_reactor_loop`]) and spawned on the runtime.
pub struct SummaryReactor {
    clock: Arc<dyn Clock>,
    runner: Arc<dyn SummaryRunner>,
    settings: Arc<dyn SettingsRepo<(), Settings>>,
    tools: Arc<dyn AgentToolRepo>,
    snapshots: Arc<dyn OutputSnapshot>,
    bus: EventBus,
    events: broadcast::Receiver<DomainEvent>,
    /// When each agent was last attempted, for the per-agent [`COOLDOWN`]; pruned when a process
    /// leaves the registry so it never outgrows the live set.
    last_attempt: HashMap<ProcessId, Instant>,
}

impl SummaryReactor {
    /// Builds a reactor over the summarizer runner, the settings and tool-registry reads, and the
    /// snapshot source, subscribing to the bus. It reads the opt-in live on each idle transition,
    /// so toggling summarization in settings takes effect without a restart.
    pub fn new(
        clock: Arc<dyn Clock>,
        runner: Arc<dyn SummaryRunner>,
        settings: Arc<dyn SettingsRepo<(), Settings>>,
        tools: Arc<dyn AgentToolRepo>,
        snapshots: Arc<dyn OutputSnapshot>,
        bus: EventBus,
    ) -> Self {
        let events = bus.subscribe();
        Self {
            clock,
            runner,
            settings,
            tools,
            snapshots,
            bus,
            events,
            last_attempt: HashMap::new(),
        }
    }

    /// Runs until the bus closes (app shutdown). Each agent that goes idle may yield one summary; a
    /// lagged subscriber simply misses a transition (best-effort) and re-summarizes on the next.
    pub async fn run(mut self) {
        loop {
            match self.events.recv().await {
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(_)) => continue,
                Ok(DomainEvent::AgentActivityChanged {
                    id,
                    state: AgentActivity::Idle,
                }) => self.on_idle(id).await,
                Ok(DomainEvent::ProcessRemoved { id }) => {
                    self.last_attempt.remove(&id);
                }
                Ok(_) => {}
            }
        }
    }

    /// Produces and publishes a summary for a just-idled agent, if summarization is enabled and the
    /// configured provider supports a headless one-shot. Silent on every degradation path.
    async fn on_idle(&mut self, id: ProcessId) {
        if self.cooling_down(id) {
            return;
        }
        let Some(invocation) = self.plan(id) else {
            return;
        };
        // Record the attempt before running, so a slow or failing summarizer still cools down and
        // is not retried on every idle tick.
        self.last_attempt.insert(id, self.clock.now());
        let runner = self.runner.clone();
        let Ok(output) = run_blocking(move || runner.run(&invocation)).await else {
            return;
        };
        let text = clamp(output.trim(), MAX_SUMMARY_CHARS);
        if !text.is_empty() {
            self.bus.publish(DomainEvent::AgentSummary { id, text });
        }
    }

    /// Whether `id` was attempted within the [`COOLDOWN`] window.
    fn cooling_down(&self, id: ProcessId) -> bool {
        self.last_attempt
            .get(&id)
            .is_some_and(|at| self.clock.now().saturating_duration_since(*at) < COOLDOWN)
    }

    /// The invocation to run for `id`, reading the live opt-in and tool registry — or `None` on any
    /// degradation path (see [`plan_summary`]).
    fn plan(&self, id: ProcessId) -> Option<SummaryInvocation> {
        let agents = self.settings.load(&()).ok()?.unwrap_or_default().agents;
        let tools = self.tools.list().ok()?;
        let snapshot = self.snapshots.recent_lines(id, SNAPSHOT_LINES);
        plan_summary(&agents, &tools, &snapshot)
    }
}

/// The pure decision: the invocation to run given the opt-in, the tool registry, and the snapshot —
/// or `None` when summarization is off (no tool named), the configured tool is not registered, its
/// provider has no headless one-shot, or there is no output to summarize.
fn plan_summary(
    agents: &AgentSettings,
    tools: &[AgentTool],
    snapshot: &[String],
) -> Option<SummaryInvocation> {
    let name = agents.summarizer_tool.as_deref()?;
    let tool = tools.iter().find(|tool| tool.name == name)?;
    if snapshot.is_empty() {
        return None;
    }
    let prompt = build_prompt(snapshot);
    summary_strategy_for(tool.kind).invocation(tool, agents.summarizer_model.as_deref(), &prompt)
}

/// Truncates `text` to at most `max` characters, on a char boundary.
fn clamp(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((byte, _)) => text[..byte].to_string(),
        None => text.to_string(),
    }
}

#[cfg(test)]
#[path = "reactor_tests.rs"]
mod tests;
