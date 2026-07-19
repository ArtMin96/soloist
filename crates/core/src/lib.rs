//! Soloist's domain core: bounded contexts, hexagonal port traits, domain types,
//! and the event bus.
//!
//! This crate is pure and framework-free — it imports no `tauri`, `rmcp`, `axum`,
//! or `rusqlite`. OS, UI, transport, and storage concerns live in adapter crates
//! behind ports; the dependency-direction check enforces this.
//!
//! Bounded contexts own their own behaviour and the port traits they drive it through;
//! adapters reach all of it via the single [`facade::Facade`], and observe it via the
//! event bus ([`events::EventBus`]). The composition root is the one place a real
//! adapter is chosen over a `Noop`.

// The core must not panic in long-running tasks: unwrap/expect/panic are denied
// outside test builds.
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

pub mod agents;
pub mod composition;
pub mod config;
pub mod configchange;
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
pub mod orchestration;
pub mod orphans;
pub mod ports;
pub mod portscan;
pub mod process;
pub mod projects;
pub mod settings;
pub mod shellenv;
pub mod supervisor;
pub mod support;
pub mod template;
pub mod terminal;
pub mod trust;

mod cache;
mod supervision;
mod sync;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use agents::{
    AgentActivity, AgentKind, AgentTool, AgentToolRepo, Agents, DetectedTool, NoopAgentToolRepo,
    NoopVersionProbe, PromptMode, VersionProbe,
};
pub use composition::{CorePorts, CorePortsBuilder};
pub use config::{
    check_command, check_command_name, ConfigEngine, ConfigError, ConfigWriteError, InvalidCommand,
    ProcessSpec, SoloYml, SyncError,
};
pub use configchange::{ConfigSync, Rename, TrustReviewCommand};
pub use coordination::{
    is_link, placeholders, AcquireOutcome, Comment, CommentAuthor, CommentEdit, CommentOutcome,
    ExportedTemplate, FireCond, IdleMode, Kv, KvEntry, KvRepo, LeaseReleaser, LeaseView, Leases,
    Link, LinkContent, LinkError, LinkTarget, LockRepo, NewTimer, NoopKvRepo, NoopLockRepo,
    NoopScratchpadRepo, NoopTemplateRepo, NoopTimerRepo, NoopTodoRepo, RenameError, RenameResult,
    ScratchpadLink, ScratchpadRef, ScratchpadRepo, ScratchpadSummary, ScratchpadTransfer,
    ScratchpadView, Scratchpads, SetWhenIdleOutcome, StoredLease, StoredScratchpad, StoredTemplate,
    StoredTimer, StoredTodo, TemplateRepo, TemplateSummary, TemplateView, TemplateWriteResult,
    Templates, TimerRepo, TimerScheduler, TimerStatus, TimerView, Timers, TodoDoc, TodoError,
    TodoLockReleaser, TodoRepo, TodoStatus, TodoSummary, TodoView, TodoWriteResult, Todos,
    TransferResult, TransferredScratchpad, WriteError, WriteResult,
};
pub use debounce::Debouncer;
pub use events::{DomainEvent, EventBus};
pub use facade::{
    CoordinationError, Facade, LaunchAgentError, LocalCommandError, MoveCommandError,
    ScopedActionError, ScopedFacade, SetupIntegrationError, SpawnAgentError, StatusSummary,
    TrustCommandError,
};
pub use filewatch::{FileWatcher, NoopFileWatcher, NoopWatchHandle, WatchHandle, WatchReactor};
pub use hash::{content_hash, Hash, HashParseError, Hasher};
pub use identity::{Identity, IdentityError, Origin, Whoami};
pub use ids::{
    ProcessId, ProjectId, ScratchpadId, SessionId, TemplateId, TimerId, TodoId, PROCESS_ID_ENV,
};
pub use metrics::{MetricsProbe, MetricsSampler, NoopMetricsProbe, ProcessMetrics};
pub use notify::{NoopNotifier, Notification, NotificationReactor, Notifier};
pub use orchestration::{AgentNode, AgentSignal, LineageEdge, OrchestrationSnapshot};
pub use orphans::{OrphanInfo, OrphanReport};
pub use ports::{
    Clock, CompositeLockReleaser, ExitFuture, ExitStatus, LockReleaser, NoopLockReleaser,
    NoopOrphanControl, NoopRuntimeState, OrphanControl, OrphanRecord, ProcessControl,
    ProcessIdentity, ProcessSpawner, ProjectRecord, ProjectRepo, PtyIo, PtySize, RuntimeState,
    RuntimeStateError, SpawnError, SpawnSpec, Spawned, StoreError, TokioClock, TrustRepo,
};
pub use portscan::{wait_for_port, NoopPortProbe, PortProbe, PortScanner, WaitForPortError};
pub use process::{IllegalTransition, ProcStatus, ProcessKind, ProcessView, Readiness};
pub use projects::{
    ConfigStatus, ConfigWatchReactor, LoadProjectError, ProjectCommandView, ProjectError,
    ProjectLoad, ProjectRef, ProjectService, ProjectSettingsPage, ProjectView, Projects,
    ReloadError, RemoveProjectError, Visibility,
};
pub use settings::{
    Appearance, Binding, FontScale, FontWeight, HotkeyAction, HotkeyBindingView, HotkeyScope,
    Hotkeys, Integrations, LetterSpacing, LineHeight, McpFeatureGroup, McpToolGroups,
    NoopSettingsRepo, Notifications, ProcessCpuThreshold, ProcessMemThreshold, ProjectSettings,
    Settings, SettingsRepo, SettingsStore, Sidebar, TemplateDefaults, TerminalAppearance, Theme,
    ToolDefaults,
};
pub use shellenv::{NoopShellEnvProbe, ShellEnvError, ShellEnvProbe};
pub use supervisor::{Registration, StartSummary, Supervisor, SupervisorError, SupervisorPorts};
pub use support::{
    agent_guide, help_overview, help_topic, onboarding_hint, Feedback, FeedbackEntry,
    FeedbackError, FeedbackRepo, IntegrationFile, IntegrationWrite, IntegrationWriteError,
    NoopFeedbackRepo,
};
pub use template::{TemplateKind, TemplateScope};
pub use terminal::{LogLine, PtyChunk, RenderedScreen};
pub use trust::{Trust, TrustStore};
