//! The orchestration read-model (a query projection over contexts C2/C4/C6).
//!
//! [`OrchestrationSnapshot`] is the single read the orchestration UI renders: the project's agent
//! lineage tree plus its coordination state (todos, timers, leases, scratchpads, key-value). It is
//! **derived on read** from the live process registry (C2), the idle tracker (C4), and the durable
//! coordination aggregates (C6) — never a separately stored copy of that state, so it cannot drift
//! from the source of truth. The Facade assembles it ([`crate::facade::Facade::orchestration_snapshot`]);
//! adapters project it and re-read it when a coordination [`DomainEvent`](crate::events::DomainEvent)
//! signals a change, rather than carrying domain payloads on every event.

use serde::Serialize;

use crate::agents::AgentActivity;
use crate::coordination::{KvEntry, LeaseView, ScratchpadSummary, TimerView, TodoView};
use crate::ids::{ProcessId, ProjectId};
use crate::process::{ProcStatus, ProcessKind};

/// One node in the agent lineage tree: a managed process the orchestration UI shows, with its
/// display label, supervision status, and — for agents — its live idle activity. `parent` is the
/// agent that spawned it (a worker under its lead), or `None` for a manually launched agent, a
/// command, or a terminal — those render as roots. A node whose parent has left the registry is
/// re-rooted on read, so a closed lead never strands its workers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AgentNode {
    pub id: ProcessId,
    pub parent: Option<ProcessId>,
    /// The process's display label — the tree row's name.
    pub label: String,
    pub kind: ProcessKind,
    pub status: ProcStatus,
    /// The five-state idle activity for an [`Agent`](ProcessKind::Agent); `None` for a command or
    /// terminal (which the idle FSM does not track) or an agent not yet classified.
    pub activity: Option<AgentActivity>,
}

/// One live spawn-lineage edge: a worker and the lead that spawned it, both still in the
/// registry. The cross-project shape the sidebar joins onto its process list to nest workers
/// under their leads; an edge disappears from this read once either end leaves the registry,
/// so a closed lead re-roots its workers exactly as in [`OrchestrationSnapshot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct LineageEdge {
    pub child: ProcessId,
    pub parent: ProcessId,
}

/// One tracked agent's current idle activity, keyed by process id. The cross-project snapshot the
/// UI's signal store seeds its idle badges from, so a dropped
/// [`AgentActivityChanged`](crate::events::DomainEvent::AgentActivityChanged) during bus lag, or a
/// webview reload, recovers the true state instead of leaving an edge-triggered badge stale. Only
/// agents classified at least once appear — a still-starting agent shows its status glyph until
/// then.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct AgentSignal {
    pub id: ProcessId,
    pub activity: AgentActivity,
}

/// The orchestration read-model for one project: the agent tree and the coordination state agents
/// share to orchestrate each other. Assembled purely from existing reads, in stable order so the UI
/// diffs it cleanly. Every field is a projection — the durable source of truth stays in the C6
/// aggregates and the C2 registry.
#[derive(Clone, Debug, Serialize)]
pub struct OrchestrationSnapshot {
    pub project: ProjectId,
    /// The project's managed processes as lineage nodes, in registry order.
    pub agents: Vec<AgentNode>,
    /// Open todos as full views (blockers, comments, lock owner, the derived `blocked` flag).
    pub todos: Vec<TodoView>,
    /// Armed and paused timers in the project, ordered by id.
    pub timers: Vec<TimerView>,
    /// Live leases in the project, ordered by key.
    pub leases: Vec<LeaseView>,
    /// One-line scratchpad summaries in the project.
    pub scratchpads: Vec<ScratchpadSummary>,
    /// Key-value entries in the project, ordered by key.
    pub kv: Vec<KvEntry>,
}
