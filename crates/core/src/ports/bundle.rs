//! The core port set: a parameter object bundling the port adapters the core is
//! constructed over, kept separate from the port *traits* (`super`) so each file has
//! one purpose.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::agents::{
    AgentToolRepo, NoopAgentToolRepo, NoopSummaryRunner, NoopVersionProbe, SummaryRunner,
    VersionProbe,
};
use crate::coordination::{
    KvRepo, LockRepo, NoopKvRepo, NoopLockRepo, NoopScratchpadRepo, NoopTimerRepo, NoopTodoRepo,
    ScratchpadRepo, TimerRepo, TodoRepo,
};
use crate::filewatch::{FileWatcher, NoopFileWatcher};
use crate::ids::ProjectId;
use crate::metrics::{MetricsProbe, NoopMetricsProbe};
use crate::notify::{NoopNotifier, Notifier};
use crate::portscan::{NoopPortProbe, PortProbe};
use crate::settings::{NoopSettingsRepo, ProjectSettings, Settings, SettingsRepo};
use crate::shellenv::{NoopShellEnvProbe, ShellEnvProbe};

use super::{
    Clock, LockReleaser, NoopLockReleaser, NoopOrphanControl, NoopRuntimeState, OrphanControl,
    ProcessSpawner, ProjectRepo, RuntimeState, TrustRepo,
};

/// The set of port adapters the core is constructed over — a parameter object so the
/// core's constructors take one value, and adding a future port is one field here
/// rather than another argument threaded through every call site. The required adapters
/// (`spawner`, `clock`, `trust`, `projects`) have no meaningful absence; the optional
/// driven subsystems (`locks`, `lock_repo`, `timer_repo`, `scratchpad_repo`, `todo_repo`,
/// `kv_repo`, `runtime`, `orphan_control`, `metrics`,
/// `port_probe`, `file_watcher`, `notifier`, `agent_tools`, `version_probe`, `summary_runner`,
/// `shell_env_probe`, `settings_repo`, `project_settings_repo`)
/// default to their `Noop` port via [`CorePorts::builder`], so a new optional port never
/// forces every existing composition root to change. `app_env` (the app's own environment,
/// captured at the composition root for the shell-environment resolver) defaults to empty.
/// The composition root (`app::build_facade`) is the one place these are chosen; tests
/// assemble it from `crate::testing` fakes.
pub struct CorePorts {
    pub(crate) spawner: Arc<dyn ProcessSpawner>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) trust: Arc<dyn TrustRepo>,
    pub(crate) projects: Arc<dyn ProjectRepo>,
    pub(crate) locks: Arc<dyn LockReleaser>,
    pub(crate) lock_repo: Arc<dyn LockRepo>,
    pub(crate) timer_repo: Arc<dyn TimerRepo>,
    pub(crate) scratchpad_repo: Arc<dyn ScratchpadRepo>,
    pub(crate) todo_repo: Arc<dyn TodoRepo>,
    pub(crate) kv_repo: Arc<dyn KvRepo>,
    pub(crate) runtime: Arc<dyn RuntimeState>,
    pub(crate) orphan_control: Arc<dyn OrphanControl>,
    pub(crate) metrics: Arc<dyn MetricsProbe>,
    pub(crate) port_probe: Arc<dyn PortProbe>,
    pub(crate) file_watcher: Arc<dyn FileWatcher>,
    pub(crate) notifier: Arc<dyn Notifier>,
    pub(crate) agent_tools: Arc<dyn AgentToolRepo>,
    pub(crate) version_probe: Arc<dyn VersionProbe>,
    pub(crate) summary_runner: Arc<dyn SummaryRunner>,
    pub(crate) shell_env_probe: Arc<dyn ShellEnvProbe>,
    pub(crate) settings_repo: Arc<dyn SettingsRepo<(), Settings>>,
    pub(crate) project_settings_repo: Arc<dyn SettingsRepo<ProjectId, ProjectSettings>>,
    pub(crate) app_env: BTreeMap<String, String>,
}

impl CorePorts {
    /// Begins a port set with the required adapters; the optional driven subsystems
    /// default to their `Noop` port until overridden on the returned builder.
    pub fn builder(
        spawner: Arc<dyn ProcessSpawner>,
        clock: Arc<dyn Clock>,
        trust: Arc<dyn TrustRepo>,
        projects: Arc<dyn ProjectRepo>,
    ) -> CorePortsBuilder {
        CorePortsBuilder {
            ports: CorePorts {
                spawner,
                clock,
                trust,
                projects,
                locks: Arc::new(NoopLockReleaser),
                lock_repo: Arc::new(NoopLockRepo),
                timer_repo: Arc::new(NoopTimerRepo),
                scratchpad_repo: Arc::new(NoopScratchpadRepo),
                todo_repo: Arc::new(NoopTodoRepo),
                kv_repo: Arc::new(NoopKvRepo),
                runtime: Arc::new(NoopRuntimeState),
                orphan_control: Arc::new(NoopOrphanControl),
                metrics: Arc::new(NoopMetricsProbe),
                port_probe: Arc::new(NoopPortProbe),
                file_watcher: Arc::new(NoopFileWatcher),
                notifier: Arc::new(NoopNotifier),
                agent_tools: Arc::new(NoopAgentToolRepo),
                version_probe: Arc::new(NoopVersionProbe),
                summary_runner: Arc::new(NoopSummaryRunner),
                shell_env_probe: Arc::new(NoopShellEnvProbe),
                settings_repo: Arc::new(NoopSettingsRepo),
                project_settings_repo: Arc::new(NoopSettingsRepo),
                app_env: BTreeMap::new(),
            },
        }
    }
}

/// Builder for [`CorePorts`]: override the optional driven subsystems, then `build`.
pub struct CorePortsBuilder {
    ports: CorePorts,
}

impl CorePortsBuilder {
    /// Overrides the lock releaser (coordination C6; defaults to [`NoopLockReleaser`]).
    pub fn locks(mut self, locks: Arc<dyn LockReleaser>) -> Self {
        self.ports.locks = locks;
        self
    }

    /// Overrides the durable lease store the coordination aggregate persists to (C6; defaults to
    /// [`NoopLockRepo`], which stores nothing). The real adapter is SQLite, shared with the
    /// lock releaser so a release is seen by every reader.
    pub fn lock_repo(mut self, lock_repo: Arc<dyn LockRepo>) -> Self {
        self.ports.lock_repo = lock_repo;
        self
    }

    /// Overrides the durable timer store the coordination scheduler persists to (C6; defaults to
    /// [`NoopTimerRepo`], which stores nothing, so no timer ever fires). The real adapter is
    /// SQLite, the same store backing every other durable repository.
    pub fn timer_repo(mut self, timer_repo: Arc<dyn TimerRepo>) -> Self {
        self.ports.timer_repo = timer_repo;
        self
    }

    /// Overrides the durable scratchpad store the coordination aggregate persists to (C6; defaults
    /// to [`NoopScratchpadRepo`], which stores nothing). The real adapter is SQLite, the same store
    /// backing every other durable repository; scratchpads are durable shared content that survives
    /// a restart.
    pub fn scratchpad_repo(mut self, scratchpad_repo: Arc<dyn ScratchpadRepo>) -> Self {
        self.ports.scratchpad_repo = scratchpad_repo;
        self
    }

    /// Overrides the durable todo store the coordination aggregate persists to (C6; defaults to
    /// [`NoopTodoRepo`], which stores nothing). The real adapter is SQLite, the same store backing
    /// every other durable repository; todos are durable shared content that survives a restart,
    /// though their process-owned locks are cleared on launch.
    pub fn todo_repo(mut self, todo_repo: Arc<dyn TodoRepo>) -> Self {
        self.ports.todo_repo = todo_repo;
        self
    }

    /// Overrides the durable key-value store the coordination aggregate persists to (C6; defaults
    /// to [`NoopKvRepo`], which stores nothing). The real adapter is SQLite, the same store backing
    /// every other durable repository; kv entries are durable shared content that survives a
    /// restart and have no process ownership.
    pub fn kv_repo(mut self, kv_repo: Arc<dyn KvRepo>) -> Self {
        self.ports.kv_repo = kv_repo;
        self
    }

    /// Overrides the runtime-state recorder for orphan adoption (defaults to
    /// [`NoopRuntimeState`]).
    pub fn runtime(mut self, runtime: Arc<dyn RuntimeState>) -> Self {
        self.ports.runtime = runtime;
        self
    }

    /// Overrides the orphan group control for adoption (defaults to
    /// [`NoopOrphanControl`]).
    pub fn orphan_control(mut self, orphan_control: Arc<dyn OrphanControl>) -> Self {
        self.ports.orphan_control = orphan_control;
        self
    }

    /// Overrides the CPU/memory probe the metrics sampler reads (monitoring C5; defaults
    /// to [`NoopMetricsProbe`], which produces no readings).
    pub fn metrics(mut self, metrics: Arc<dyn MetricsProbe>) -> Self {
        self.ports.metrics = metrics;
        self
    }

    /// Overrides the port probe the port scanner reads (monitoring C5; defaults to
    /// [`NoopPortProbe`], which discovers nothing).
    pub fn port_probe(mut self, port_probe: Arc<dyn PortProbe>) -> Self {
        self.ports.port_probe = port_probe;
        self
    }

    /// Overrides the file watcher the file-watch reactor reads (monitoring C5; defaults to
    /// [`NoopFileWatcher`], which watches nothing — so the reactor never restarts).
    pub fn file_watcher(mut self, file_watcher: Arc<dyn FileWatcher>) -> Self {
        self.ports.file_watcher = file_watcher;
        self
    }

    /// Overrides the desktop notifier the notification reactor shows toasts through
    /// (notifications C7; defaults to [`NoopNotifier`], which shows nothing).
    pub fn notifier(mut self, notifier: Arc<dyn Notifier>) -> Self {
        self.ports.notifier = notifier;
        self
    }

    /// Overrides the durable agent-tool registry (agents C4; defaults to
    /// [`NoopAgentToolRepo`], an empty registry). The real adapter (SQLite) seeds the
    /// built-in providers on first run.
    pub fn agent_tools(mut self, agent_tools: Arc<dyn AgentToolRepo>) -> Self {
        self.ports.agent_tools = agent_tools;
        self
    }

    /// Overrides the `--version` probe used to auto-detect installed agent CLIs (agents C4;
    /// defaults to [`NoopVersionProbe`], which detects nothing).
    pub fn version_probe(mut self, version_probe: Arc<dyn VersionProbe>) -> Self {
        self.ports.version_probe = version_probe;
        self
    }

    /// Overrides the headless summarizer executor the summary reactor runs invocations through
    /// (agents C4; defaults to [`NoopSummaryRunner`], which produces no summary, so opting into
    /// auto-summarization stores the preference but yields nothing until the real adapter is
    /// wired — the core never hard-depends on an LLM).
    pub fn summary_runner(mut self, summary_runner: Arc<dyn SummaryRunner>) -> Self {
        self.ports.summary_runner = summary_runner;
        self
    }

    /// Overrides the login-shell environment probe the supervisor captures version-manager
    /// PATHs through (defaults to [`NoopShellEnvProbe`], which captures nothing, so a process
    /// gets exactly its own `env` over the inherited app environment). The real adapter runs
    /// `$SHELL -ilc env`.
    pub fn shell_env_probe(mut self, shell_env_probe: Arc<dyn ShellEnvProbe>) -> Self {
        self.ports.shell_env_probe = shell_env_probe;
        self
    }

    /// Overrides the durable settings store the settings aggregate persists to (defaults to
    /// [`NoopSettingsRepo`], which stores nothing, so settings stay at their defaults). The real
    /// adapter is SQLite, the same store backing every other durable repository.
    pub fn settings_repo(mut self, settings_repo: Arc<dyn SettingsRepo<(), Settings>>) -> Self {
        self.ports.settings_repo = settings_repo;
        self
    }

    /// Overrides the durable per-project local-settings store the per-project settings aggregate
    /// persists to (defaults to [`NoopSettingsRepo`], which stores nothing, so per-project settings
    /// stay at their defaults). The real adapter is SQLite, the same store backing every other
    /// durable repository; the same generic port keyed by `ProjectId` rather than `()`.
    pub fn project_settings_repo(
        mut self,
        project_settings_repo: Arc<dyn SettingsRepo<ProjectId, ProjectSettings>>,
    ) -> Self {
        self.ports.project_settings_repo = project_settings_repo;
        self
    }

    /// Sets the app's own environment, captured once at the composition root. The
    /// shell-environment resolver uses it as the base layer and the source of the fallback
    /// `PATH` when a shell capture fails (defaults to empty).
    pub fn app_env(mut self, app_env: BTreeMap<String, String>) -> Self {
        self.ports.app_env = app_env;
        self
    }

    /// Finishes the port set.
    pub fn build(self) -> CorePorts {
        self.ports
    }
}
