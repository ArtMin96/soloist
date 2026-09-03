//! The typed domain event bus.
//!
//! Contexts publish [`DomainEvent`]s — deltas over a snapshot — onto a bounded
//! `tokio::sync::broadcast` channel; adapters subscribe and project them into their
//! own read models. The contract is **snapshot-then-deltas**: an adapter first reads
//! a full snapshot (e.g. [`crate::facade::Facade::snapshot`]), then applies events.
//! If a slow subscriber lags and the channel drops messages, `recv` reports
//! `Lagged`; the adapter recovers by re-reading the snapshot rather than trusting a
//! gap-filled stream. The channel is bounded so a stalled subscriber can never grow
//! memory without limit.

use std::collections::BTreeMap;

use serde::Serialize;
use tokio::sync::broadcast;

use crate::attention::AttentionKind;
use crate::configchange::{ConfigSync, TrustReviewCommand};
use crate::idle::AgentActivity;
use crate::ids::{AgentMessageId, ProcessId, ProjectId, TimerId, TodoId, TrustRequestId};
use crate::orphans::OrphanInfo;
use crate::process::{ProcStatus, ProcessKind};
use crate::template::TemplateKind;
use crate::trustrequest::{TrustRequest, TrustRequestState};
use crate::watch::{WatchLimit, WatchPurpose};

/// A change in domain state, serialized to adapters verbatim. `#[serde(tag = "type")]`
/// gives each variant a discriminator field so a JS/TS consumer can switch on it.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    /// A new process entered the registry (initial status included). `requires_trust`
    /// is true for a trust-gated command whose variant is not yet trusted — the UI
    /// blocks its start and offers a trust affordance.
    ProcessSpawned {
        id: ProcessId,
        project: ProjectId,
        kind: ProcessKind,
        label: String,
        status: ProcStatus,
        requires_trust: bool,
        /// True for an agent whose provider supports resuming its last session (see
        /// [`crate::process::ProcessView::resumable`]); the UI offers "Resume last session"
        /// when it rests. Carried on spawn so a row created from this event — not only from a
        /// snapshot — knows it.
        resumable: bool,
    },
    /// A process moved between lifecycle states. `exit_code` is set on a terminal
    /// transition driven by the child exiting on its own (`None` when terminated by a
    /// signal or for non-terminal transitions).
    ProcessStatusChanged {
        id: ProcessId,
        from: ProcStatus,
        to: ProcStatus,
        exit_code: Option<i32>,
    },
    /// A process left the registry.
    ProcessRemoved { id: ProcessId },
    /// A process's display label changed. The new label is carried so adapters update the
    /// row without a snapshot round-trip; trust (keyed on the command variant) and identity
    /// are unaffected — the label is display-only.
    ProcessRenamed { id: ProcessId, label: String },
    /// A periodic CPU/memory reading for a running process, sampled across its whole
    /// process group. `cpu_pct` is normalised to the whole machine (100 = every core busy,
    /// never above); `rss` is the group's memory in bytes, shared pages counted once.
    /// Emitted on the sampler's interval when the reading changed since the last tick for that
    /// process, plus an occasional heartbeat so a steady process (an idle server) is re-published
    /// now and then rather than falling silent forever — a subscriber that mounts or reloads after
    /// the reading last moved has no snapshot to seed from, and the heartbeat repopulates it. So
    /// adapters see roughly one event per *change*, not per process per second. A single late
    /// reading may arrive just after a process stops (sampled before it exited); it carries no view
    /// state, so consumers simply ignore one for a process no longer running.
    MetricsTick {
        id: ProcessId,
        cpu_pct: f32,
        rss: u64,
    },
    /// A process's set of bound (listening) TCP ports changed — discovered while it runs,
    /// emptied when it stops. The new sorted set is carried so adapters update the read
    /// model without a snapshot round-trip; it is also reflected on [`ProcessView::ports`].
    PortsChanged { id: ProcessId, ports: Vec<u16> },
    /// A process's readiness changed while a port wait is in effect: `false` = Running but
    /// the awaited port has not bound yet ("Running but not Ready"), `true` = it bound. Only
    /// fired while a readiness gate is active; reflected on [`ProcessView::ready`].
    ReadyStateChanged { id: ProcessId, ready: bool },
    /// The restart policy is relaunching a crashed `auto_restart` command. `attempt` is
    /// its position in the current rate-limit window (1 = the first restart), and `limit`
    /// is how many that window allows before the command is held exhausted. The status
    /// also moves `Crashed -> Starting`; this delta additionally carries both numbers for
    /// the "restarting (k/N)" affordance and crash notifications.
    ///
    /// `limit` rides along with every event so the policy stays the core's alone: a
    /// display that renders `attempt/limit` never has to know the gate's value to
    /// describe it, and cannot fall out of step with it.
    RestartScheduled {
        id: ProcessId,
        attempt: u32,
        limit: u32,
    },
    /// The restart policy gave up on a command that crashed too fast, too often: it is
    /// held in [`ProcStatus::RestartExhausted`] until the user restarts it. Distinct from
    /// the status delta so notifications can fire on it.
    RestartExhausted { id: ProcessId },
    /// A command was restarted because a watched file changed (the file-watch policy). The
    /// status also cycles through the usual restart deltas; this discrete signal lets the UI
    /// distinguish a file-watch restart (a banner/notification) from a crash or user restart.
    FileRestart { id: ProcessId },
    /// A project was opened (or its set of projects changed). Carries the project's id; an
    /// adapter re-reads the project read model ([`crate::projects::ProjectView`], which
    /// resolves name and icon together) rather than carrying that display state on the event.
    ProjectOpened { id: ProjectId },
    /// A project was removed from the registry: its processes were closed (each announcing
    /// [`Self::ProcessRemoved`]) and its durable project-scoped state deleted. Adapters
    /// re-read the project read model and drop any state keyed to the id. Files on disk
    /// are untouched.
    ProjectRemoved { id: ProjectId },
    /// The user rearranged the project list. The new order is durable and carried by the
    /// project read model itself, so adapters re-read that model rather than reconstructing
    /// the order from the event.
    ProjectsReordered,
    /// A project's `solo.yml` changed on disk. Carries the add/update/remove/rename
    /// diff, whether any added/updated command now needs (re-)trust, and the detail of
    /// each command awaiting trust (so the review dialog can show what will run). Sync
    /// never starts a process — this event only informs adapters of the change.
    ConfigChanged {
        project: ProjectId,
        diff: ConfigSync,
        requires_trust: bool,
        commands: Vec<TrustReviewCommand>,
    },
    /// A bound process asked the user to trust a command variant it wants to run. Carries the
    /// whole request, so the approval surface shows what would run, who is asking, and the words
    /// they gave — without a round trip, and without the user ever seeing only a name.
    ///
    /// The reason is **agent-supplied text**: attribute it to the named requester and render it as
    /// plain text, never as the application's own words and never as markup.
    TrustRequested {
        project: ProjectId,
        request: TrustRequest,
    },
    /// A pending trust request reached a terminal state — approved, declined, aged out, or dropped
    /// because the process that asked has closed. An approval prompt on screen for it closes.
    TrustRequestResolved {
        project: ProjectId,
        id: TrustRequestId,
        state: TrustRequestState,
    },
    /// A process set its terminal title via an OSC sequence. Drives window/tab titles
    /// and feeds the agent idle heuristics that watch title stability.
    TerminalTitleChanged { id: ProcessId, title: String },
    /// A process rang the terminal bell (`BEL`). Drives attention notifications.
    TerminalBell { id: ProcessId },
    /// A process raised a notification of its own from its output, via one of the escape
    /// sequences that carry notification text (OSC 9, 777, or 99).
    ///
    /// Unlike the change-notifications around it this carries the words the script chose,
    /// deliberately: they exist nowhere else, so there is no read model to re-query for them.
    /// `title` is `None` when the sequence carried only a message, leaving the surface to name
    /// the process instead.
    TerminalNotification {
        id: ProcessId,
        title: Option<String>,
        body: String,
    },
    /// An agent process's activity changed (the five-state idle FSM). Emitted only on a
    /// transition (edge-triggered), so adapters update the agent's row without polling.
    /// `Permission` and `Error` are attention states and raise a notification.
    AgentActivityChanged { id: ProcessId, state: AgentActivity },
    /// Reconciliation found leftover process groups from a previous run that match no
    /// known command, awaiting a user Kill / Kill All / Leave decision surfaced by the
    /// UI. The core only reports them; it neither kills nor keeps them on its own.
    OrphansFound { orphans: Vec<OrphanInfo> },
    /// A coordination todo in `project` changed (created, updated, completed, deleted, or one of
    /// its live columns — tags, blockers, comments, lock — was edited). A change-notification
    /// carrying ids only: the orchestration UI re-reads the snapshot rather than trusting a
    /// payload, so a chatty run coalesces to one re-query per frame.
    TodoChanged { project: ProjectId, id: TodoId },
    /// A recorded agent-to-agent exchange in `project` changed — queued, woken, or acknowledged. A
    /// change-notification carrying ids only: the orchestration UI re-reads the snapshot rather
    /// than trusting a payload, so a chatty run coalesces to one re-query per frame. The body
    /// deliberately stays off the bus — the retained record is its single source, and this bus
    /// fans out to every subscriber of a public subscription.
    AgentMessageChanged {
        project: ProjectId,
        id: AgentMessageId,
    },
    /// A coordination timer owned by `owner` was armed (created or set fire-when-idle). Carries the
    /// owner and timer id so the orchestration UI re-reads that owner's timers.
    TimerArmed { owner: ProcessId, id: TimerId },
    /// A coordination timer owned by `owner` fired: its body was delivered to the owner as a fresh
    /// turn and the timer was removed. Distinct from [`TimerCleared`](Self::TimerCleared) so a
    /// wake-cycle view can surface *why* the lead woke, not just that the timer left.
    TimerFired { owner: ProcessId, id: TimerId },
    /// A coordination timer owned by `owner` was cleared without firing (the owner cancelled it).
    TimerCleared { owner: ProcessId, id: TimerId },
    /// A coordination timer owned by `owner` was paused: the countdown is frozen and the timer
    /// will not fire until resumed.
    TimerPaused { owner: ProcessId, id: TimerId },
    /// A coordination timer owned by `owner` was resumed: the countdown re-armed with the time
    /// that remained when it was paused.
    TimerResumed { owner: ProcessId, id: TimerId },
    /// A coordination lease `key` in `project` changed (acquired, renewed, or released by its
    /// owner). The UI re-reads the project's live leases.
    LeaseChanged { project: ProjectId, key: String },
    /// A coordination scratchpad `name` in `project` changed (written, renamed, retagged, archived,
    /// or deleted). Keyed by the scratchpad's `name` handle — the addressing key its surface uses.
    ScratchpadChanged { project: ProjectId, name: String },
    /// A coordination diagram `name` in `project` changed (written, renamed, retagged, archived, or
    /// deleted). Keyed by the diagram's `name` handle — the addressing key its surface uses.
    DiagramChanged { project: ProjectId, name: String },
    /// A coordination key-value entry `key` in `project` changed (set or deleted).
    KvChanged { project: ProjectId, key: String },
    /// `process` read or wrote a todo or scratchpad through a bound session this run, for the
    /// first time or with a new [`AccessKind`](crate::coordination::AccessKind). Ids only, like the
    /// other change-notifications: a subscriber re-reads
    /// [`Facade::session_work`](crate::facade::Facade::session_work) rather than trusting a
    /// payload, so a chatty run coalesces to one re-query per frame.
    SessionWorkChanged { process: ProcessId },
    /// An alert was raised for a user who is looking at Soloist but not at the process that
    /// raised it, so it belongs in an in-app toast rather than a desktop notification. The
    /// notification reactor has already applied the master switch, the notification level, and
    /// the focus rules ([`crate::notify::route`]); a surface renders this and decides nothing.
    ///
    /// Unlike the change-notifications around it this carries its composed text, deliberately.
    /// A notification is transient — there is no record to re-query once it has been raised —
    /// and composing the same title and body once in Rust for the desktop and again in
    /// TypeScript for the toast would make one sentence two sources of truth.
    NotificationRaised {
        process: ProcessId,
        kind: AttentionKind,
        title: String,
        body: String,
        /// A sound for the surface to play, or `None` to show silently. A hint only — see
        /// [`crate::notify::Notification::sound`].
        sound: Option<String>,
    },
    /// The set of processes with unread attention changed. Payload-free by the same convention as
    /// the other change-notifications: a consumer re-reads
    /// [`Facade::attention_snapshot`](crate::facade::Facade::attention_snapshot), so the several
    /// surfaces that render unread cannot drift apart by projecting a payload differently.
    AttentionChanged,
    /// Where the user is changed: the window gained or lost focus, or it now shows a different
    /// process. Payload-free by the same convention as the other change-notifications: a consumer
    /// re-reads [`Facade::presence`](crate::facade::Facade::presence).
    ///
    /// The app-icon badge is the surface that needs this. What it draws turns on whether the user
    /// is at the window, so it has to be told about a walk away from a stack of unread that
    /// changed nothing about the unread itself.
    PresenceChanged,
    /// A template of `kind` was created, updated, or deleted. `project` names the scope it
    /// changed in — `None` for the global library, `Some` for that project's — because the two
    /// scopes are separate libraries a surface reads separately: without it a project-scoped
    /// write would make a listener re-read the global list and see nothing. A low-frequency
    /// change-notification carrying no content: the surface re-reads that `(kind, scope)`'s list
    /// rather than trusting a payload, and the selected default is read separately.
    TemplateChanged {
        kind: TemplateKind,
        project: Option<ProjectId>,
    },
    /// A project's working tree or its standing against its upstream changed. A
    /// change-notification carrying the project only: the surface re-reads
    /// [`Facade::git_status`](crate::facade::Facade::git_status) rather than trusting a
    /// payload, so a repository under active change coalesces to one re-query per frame
    /// instead of one per file.
    GitStatusChanged { project: ProjectId },
    /// Which of a project's watches is limited changed: `limits` names what each affected
    /// [`WatchPurpose`] met — refused outright, or degraded to its essential watches — and is
    /// empty once every watch it had limited is established again in full.
    ///
    /// Keyed by purpose because the two watches fail into different sentences — a refused restart
    /// watch stops a `restart_when_changed` command reloading on a save, a refused git watch stops
    /// a status refreshing on its own — and a project can meet one without the other, or ask for
    /// only one of them. Handed a single reason for the pair, a surface could only claim both,
    /// including on a project that declares no `restart_when_changed` and never asks for that
    /// watch at all.
    ///
    /// Edge-triggered in both directions, like [`Self::ReadyStateChanged`]: the reactors ask for a
    /// limited root again on every re-sync, so a signal per attempt would repeat one sentence for
    /// as long as the condition lasted.
    ///
    /// It carries the limits rather than pointing at a read model, deliberately. An exhausted
    /// watch budget is the user's to raise and an unreadable directory is not, and there is no
    /// record to re-query for which it was. This is the one degradation nothing else reveals — a
    /// watch that yields no events looks exactly like a tree nobody edits — so if it is not said
    /// here it is not said at all.
    WatchLimitChanged {
        project: ProjectId,
        limits: BTreeMap<WatchPurpose, WatchLimit>,
    },
}

/// The outbound event port: anything the core publishes domain events through.
///
/// Realized by [`EventBus`]. Defined as a trait so an adapter that needs a different
/// fan-out shape (e.g. an MCP push sink) can provide its own implementation without
/// the core depending on it.
pub trait EventSink: Send + Sync {
    /// Publishes an event. Best-effort: a sink with no live receivers drops it.
    fn emit(&self, event: DomainEvent);
}

/// A bounded broadcast bus carrying [`DomainEvent`]s from the core to all adapters.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
}

impl EventBus {
    /// Creates a bus whose channel buffers at most `capacity` undelivered events per
    /// subscriber before the slowest subscriber starts observing `Lagged`.
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribes a new receiver. Adapters pair this with a fresh snapshot read.
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }

    /// Publishes an event; dropped silently when no subscribers are attached.
    pub fn publish(&self, event: DomainEvent) {
        let _ = self.tx.send(event);
    }
}

impl EventSink for EventBus {
    fn emit(&self, event: DomainEvent) {
        self.publish(event);
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
