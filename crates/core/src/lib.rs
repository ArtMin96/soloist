//! Soloist's domain core: bounded contexts, hexagonal port traits, domain types,
//! and the event bus.
//!
//! This crate is pure and framework-free — it imports no `tauri`, `rmcp`, `axum`,
//! or `rusqlite`. OS, UI, transport, and storage concerns live in adapter crates
//! behind ports; the dependency-direction check enforces this.
//!
//! The walking skeleton wires three live ports ([`ports::ProcessSpawner`],
//! [`ports::Clock`], [`ports::Store`]) and the event bus ([`events::EventBus`])
//! through a single [`facade::Facade`], proving the architecture end to end before
//! any feature lands.

// The core must not panic in long-running tasks: unwrap/expect/panic are denied
// outside test builds.
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod agents;
pub mod config;
pub mod coordination;
pub mod debounce;
pub mod events;
pub mod facade;
pub mod filewatch;
pub mod hash;
pub mod identity;
pub mod idle;
pub mod ids;
pub mod metrics;
pub mod notify;
pub mod orphans;
pub mod ports;
pub mod portscan;
pub mod process;
pub mod projects;
pub mod settings;
pub mod shellenv;
pub mod supervisor;
pub mod terminal;
pub mod trust;

mod supervision;
mod sync;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use agents::{
    AgentKind, AgentTool, AgentToolRepo, Agents, DetectedTool, NoopAgentToolRepo, NoopVersionProbe,
    PromptMode, VersionProbe,
};
pub use config::{
    ConfigEngine, ConfigError, ConfigSync, ProcessSpec, Rename, SoloYml, SyncError,
    TrustReviewCommand,
};
pub use coordination::{
    AcquireOutcome, Comment, CommentEdit, CommentOutcome, FireCond, IdleMode, Kv, KvEntry, KvRepo,
    LeaseReleaser, LeaseView, Leases, LockRepo, NewTimer, NoopKvRepo, NoopLockRepo,
    NoopScratchpadRepo, NoopTimerRepo, NoopTodoRepo, RenameError, RenameResult, ScratchpadDoc,
    ScratchpadRepo, ScratchpadSummary, ScratchpadView, Scratchpads, SetWhenIdleOutcome,
    StoredLease, StoredScratchpad, StoredTimer, StoredTodo, TimerRepo, TimerScheduler, TimerStatus,
    TimerView, Timers, TodoDoc, TodoError, TodoLockReleaser, TodoRepo, TodoStatus, TodoSummary,
    TodoView, TodoWriteResult, Todos, WriteError, WriteResult,
};
pub use debounce::Debouncer;
pub use events::{DomainEvent, EventBus};
pub use facade::{
    CoordinationError, Facade, LaunchAgentError, ScopedActionError, SpawnAgentError,
    TrustCommandError,
};
pub use filewatch::{FileWatcher, NoopFileWatcher, NoopWatchHandle, WatchHandle, WatchReactor};
pub use hash::{content_hash, Hash, HashParseError, Hasher};
pub use identity::{Identity, IdentityError, Origin, Whoami, PROCESS_ID_ENV};
pub use ids::{ProcessId, ProjectId, ScratchpadId, SessionId, TimerId, TodoId};
pub use metrics::{MetricsProbe, MetricsSampler, NoopMetricsProbe, ProcessMetrics};
pub use notify::{NoopNotifier, Notification, NotificationReactor, Notifier};
pub use orphans::{OrphanInfo, OrphanReport};
pub use ports::{
    Clock, CompositeLockReleaser, CorePorts, CorePortsBuilder, ExitFuture, ExitStatus,
    LockReleaser, NoopLockReleaser, NoopOrphanControl, NoopRuntimeState, OrphanControl,
    OrphanRecord, ProcessControl, ProcessSpawner, ProjectRecord, ProjectRepo, PtyIo, PtySize,
    RuntimeState, RuntimeStateError, SpawnError, SpawnSpec, Spawned, Store, StoreError, TokioClock,
    TrustRepo,
};
pub use portscan::{wait_for_port, NoopPortProbe, PortProbe, PortScanner, WaitForPortError};
pub use process::{IllegalTransition, ProcStatus, ProcessKind, ProcessView, Readiness};
pub use projects::{
    LoadProjectError, ProjectError, ProjectLoad, ProjectService, ProjectView, Projects,
};
pub use settings::{
    AgentSettings, Appearance, FontScale, FontWeight, Integrations, LetterSpacing, LineHeight,
    McpFeatureGroup, McpToolGroups, NoopSettingsRepo, ProcessCpuThreshold, ProcessMemThreshold,
    ProjectCpuThreshold, ProjectMemThreshold, Settings, SettingsRepo, SettingsStore, Sidebar,
    TerminalAppearance, Theme, ToolDefaults,
};
pub use shellenv::{NoopShellEnvProbe, ShellEnvError, ShellEnvProbe};
pub use supervisor::{Registration, StartSummary, Supervisor, SupervisorError};
pub use terminal::{LogLine, PtyChunk, RenderedScreen};
pub use trust::{Trust, TrustStore};
