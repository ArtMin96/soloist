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
pub mod attention;
pub mod composition;
pub mod config;
pub mod configchange;
pub mod coordination;
pub mod debounce;
pub mod events;
pub mod facade;
pub mod filewatch;
pub mod git;
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
pub mod vcs;
pub mod watch;

mod cache;
mod supervision;
mod sync;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use agents::{
    AgentActivity, AgentKind, AgentOneShot, AgentTool, AgentToolRepo, Agents, DetectedTool,
    Detection, NoopAgentOneShot, NoopAgentToolRepo, NoopVersionProbe, OneShotError,
    OneShotInvocation, PromptMode, VersionProbe, ONE_SHOT_PROMPT_LIMIT, ONE_SHOT_REPLY_LIMIT,
};
pub use composition::{CorePorts, CorePortsBuilder};
pub use config::{
    check_command, check_command_name, ConfigEngine, ConfigError, ConfigWriteError, InvalidCommand,
    ProcessSpec, SoloYml, SyncError,
};
pub use configchange::{ConfigSync, Rename, TrustReviewCommand};
pub use coordination::{
    is_link, placeholders, AcquireOutcome, AgentBroadcastReceipt, AgentMailbox, AgentMessage,
    AgentMessageDelivery, AgentMessageKind, AgentMessageOutcome, AgentMessageReceipt,
    AgentMessageRecord, AgentRelationship, AgentRosterEntry, Comment, CommentAuthor, CommentEdit,
    CommentOutcome, DiagramRef, DiagramRenameError, DiagramRenameResult, DiagramRepo,
    DiagramSummary, DiagramView, DiagramWriteError, DiagramWriteResult, Diagrams, ExportedTemplate,
    FireCond, IdleMode, Kv, KvEntry, KvRepo, LeaseReleaser, LeaseView, Leases, Link, LinkContent,
    LinkError, LinkTarget, LockRepo, MissingPolicy, NewTimer, NoopDiagramRepo, NoopKvRepo,
    NoopLockRepo, NoopScratchpadRepo, NoopTemplateRepo, NoopTimerRepo, NoopTodoRepo, RenameError,
    RenameResult, RenderError, RenderRequest, RenderedPrompt, ScratchpadLink, ScratchpadRef,
    ScratchpadRepo, ScratchpadSummary, ScratchpadTransfer, ScratchpadView, Scratchpads,
    SeedTemplate, SetWhenIdleOutcome, StoredDiagram, StoredLease, StoredScratchpad, StoredTemplate,
    StoredTimer, StoredTodo, TemplateRepo, TemplateSummary, TemplateView, TemplateWriteResult,
    Templates, TimerRepo, TimerScheduler, TimerStatus, TimerView, Timers, TodoCompletion,
    TodoCompletionAtomicResult, TodoCompletionCompareResult, TodoCompletionContext,
    TodoCompletionDecision, TodoCompletionIntent, TodoCompletionKey, TodoCompletionNotice,
    TodoCompletionNoticeOutcome, TodoCompletionOccurrence, TodoCompletionOutcome, TodoDoc,
    TodoError, TodoLockReleaser, TodoRepo, TodoStatus, TodoSummary, TodoView, TodoWriteResult,
    Todos, TransferResult, TransferredScratchpad, WriteError, WriteResult, MAX_AGENT_MESSAGE_BYTES,
    MAX_PENDING_AGENT_MESSAGES, MAX_PENDING_AGENT_MESSAGE_BYTES, MAX_PENDING_MESSAGES_PER_PROJECT,
    MAX_PENDING_MESSAGES_PER_RECIPIENT,
};
pub use debounce::Debouncer;
pub use events::{DomainEvent, EventBus};
pub use facade::{
    AgentMailboxError, AppearanceSettingsError, CompletionNotification, CompletionReport,
    CoordinationError, CreateTerminalError, DraftError, Facade, GitReadError, Handoff,
    HandoffError, LaunchAgentError, LocalCommandError, MoveCommandError, PromptRenderError,
    ScopedActionError, ScopedFacade, ScopedGitError, SetupIntegrationError, SpawnAgentError,
    SpawnAgentOutcome, SpawnAgentRequest, SpawnProcessError, SpawnProcessRequest, StatusSummary,
    TrustCommandError,
};
pub use filewatch::{FileWatcher, NoopFileWatcher, NoopWatchHandle, WatchHandle, WatchReactor};
pub use git::{
    BranchOp, CheckRun, CheckState, DiffExtent, Exchange, FileOpener, ForgeError, ForgeReadiness,
    ForgeRepository, Git, GitDraftError, GitError, GitForge, GitRepository, GitStatus,
    GitStatusWatchReactor, GitWriteError, HandoffSubject, LogRange, MergeMethod, NewPullRequest,
    NoopFileOpener, NoopGitForge, NoopGitRepository, Observer, OpenError, Progress, Prompting,
    PullRequest, PullRequestError, PullRequestReview, PullRequestState, PullRequestSuggestion,
    PullRequestSurface, PullRequestTemplate, RawFileDiff, RawHunk, ReviewComment, ReviewLimits,
    ReviewThread, StashOp, Stop, SyncOp, BRANCH_PAGE_SIZE, CHECK_LOG_LIMIT, HANDOFF_LIMIT,
    LOG_PAGE_SIZE, REVIEW_LIMITS,
};
pub use hash::{content_hash, Hash, HashParseError, Hasher};
pub use identity::{Identity, IdentityError, Origin, PeerCredentials, Whoami};
pub use ids::{
    AgentMessageId, DiagramId, ProcessId, ProjectId, ScratchpadId, SessionId, TemplateId, TimerId,
    TodoId, PROCESS_ID_ENV,
};
pub use metrics::{MetricsProbe, MetricsSampler, NoopMetricsProbe, ProcessMetrics};
pub use notify::{
    AttentionSnapshot, NoopNotifier, Notification, NotificationReactor, Notifier, NotifierStatus,
    Presence, ProcessAttention,
};
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
    built_in_themes, default_theme_colors, soloist_default_theme, Appearance, Assist, Binding,
    CursorInactiveStyle, CursorStyle, FontScale, FontWeight, GlassOpacity, HotkeyAction,
    HotkeyBindingView, HotkeyScope, Hotkeys, Integrations, LetterSpacing, LineHeight,
    McpFeatureGroup, McpToolGroups, NoopSettingsRepo, NotificationLevel, Notifications,
    ProcessCpuThreshold, ProcessMemThreshold, ProjectSettings, SelectedThemes, Settings,
    SettingsRepo, SettingsStore, Sidebar, SoloistThemeExtensions, SoloistThemeRole,
    TemplateDefaults, TerminalAppearance, Theme, ThemeAppearance, ThemeColor, ThemeColorRole,
    ThemeColors, ThemeConflictPolicy, ThemeError, ThemeExtensions, ThemeFile, ThemeMutation,
    ThemeVariantExtensions, ThemeVariants, ToolDefaults, DEFAULT_THEME_ID, THEME_FILE_VERSION,
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
pub use vcs::{
    Branch, BranchInfo, Branches, ChangeKind, CommitEntry, DiffTarget, FileChange, FileContent,
    FileDiff, GitFileStatus, HunkRange, ProjectFile, SyncState, COMMIT_BODY_LIMIT,
};
pub use watch::{WatchError, WatchPurpose};
