//! The public command and query API that adapters call (context C8).
//!
//! [`Facade`] is the one surface every adapter (Tauri, MCP, HTTP/CLI) talks to. It
//! owns the event bus and the bounded contexts — projects/trust/config (C1), process
//! supervision (C2), terminal I/O (C3), agents & idle (C4), monitoring (C5),
//! coordination (C6: leases & timers), notifications (C7), and identity (C8) — and hands
//! adapters references to them, so a behaviour like "restart" or "is this command
//! trusted" is implemented exactly once. Adapters translate requests in and project the
//! read model out; they hold no business state.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, Notify};

use crate::agents::{Agents, IdleTracker};
use crate::config::ConfigEngine;
use crate::coordination::{Kv, Leases, Scratchpads, Timers, Todos};
use crate::events::{DomainEvent, EventBus};
use crate::filewatch::FileWatcher;
use crate::identity::Identity;
use crate::ids::{ProcessId, ProjectId};
use crate::metrics::MetricsProbe;
use crate::notify::Notifier;
use crate::ports::{Clock, CorePorts, PtySize, SpawnSpec, StoreError};
use crate::portscan::{self, PortProbe, WaitForPortError};
use crate::process::{ProcessKind, ProcessView};
use crate::projects::{LoadProjectError, ProjectLoad, ProjectService, ProjectView, Projects};
use crate::supervisor::{Registration, Supervisor, SupervisorError};
use crate::trust::TrustStore;

mod coordination;
mod kv;
mod loops;
mod output;
mod scoped;
mod scratchpad;
mod session;
mod todo;

pub use coordination::CoordinationError;
pub use scoped::{ScopedActionError, SpawnAgentError};

/// Per-subscriber event buffer. Bounded so a stalled adapter re-syncs from a snapshot
/// (see [`crate::events`]) rather than growing memory without limit.
const EVENT_BUFFER: usize = 1024;

/// The integration façade (context C8). Cheap to share as Tauri-managed state.
pub struct Facade {
    bus: EventBus,
    clock: Arc<dyn Clock>,
    metrics: Arc<dyn MetricsProbe>,
    port_probe: Arc<dyn PortProbe>,
    file_watcher: Arc<dyn FileWatcher>,
    notifier: Arc<dyn Notifier>,
    notifications_enabled: Arc<AtomicBool>,
    supervisor: Arc<Supervisor>,
    projects: Projects,
    trust: TrustStore,
    config: ConfigEngine,
    agents: Agents,
    idle: Arc<IdleTracker>,
    identity: Identity,
    kv: Kv,
    leases: Leases,
    timers: Timers,
    scratchpads: Scratchpads,
    todos: Todos,
}

impl Facade {
    /// Builds a façade over the given core port set (real adapters in the app, fakes in
    /// tests). The trust repository is shared by the supervisor's trust gate, the trust
    /// store, and the config sync engine, so all three agree on what is trusted.
    pub fn new(ports: CorePorts) -> Self {
        let bus = EventBus::new(EVENT_BUFFER);
        let supervisor = Arc::new(Supervisor::new(&ports, bus.clone()));
        let CorePorts {
            clock,
            metrics,
            port_probe,
            file_watcher,
            notifier,
            trust,
            projects,
            agent_tools,
            version_probe,
            kv_repo,
            lock_repo,
            timer_repo,
            scratchpad_repo,
            todo_repo,
            ..
        } = ports;
        Self {
            supervisor,
            kv: Kv::new(kv_repo),
            leases: Leases::new(lock_repo, clock.clone()),
            // The scheduler shares this wake handle with the aggregate (see `Timers`), so creating
            // or resuming a timer re-evaluates the schedule at once.
            timers: Timers::new(timer_repo, clock.clone(), Arc::new(Notify::new())),
            scratchpads: Scratchpads::new(scratchpad_repo),
            todos: Todos::new(todo_repo),
            clock,
            metrics,
            port_probe,
            file_watcher,
            notifier,
            // Notifications are on by default; the user can silence them at runtime.
            notifications_enabled: Arc::new(AtomicBool::new(true)),
            projects: Projects::new(projects),
            trust: TrustStore::new(trust.clone()),
            config: ConfigEngine::new(trust, bus.clone()),
            agents: Agents::new(agent_tools, version_probe),
            idle: Arc::new(IdleTracker::new()),
            identity: Identity::new(),
            bus,
        }
    }

    /// Subscribes to the domain event stream. Pair with [`Facade::snapshot`]: read the
    /// snapshot first, then apply events (snapshot-then-deltas).
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.bus.subscribe()
    }

    /// The current process read model. Cheap; never blocks writers.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        self.supervisor.snapshot()
    }

    /// One process's read-model row by id, `None` if it is no longer registered — the
    /// single-process read, so a caller after one process clones one row, not the whole
    /// [`snapshot`](Self::snapshot).
    pub fn process_view(&self, id: ProcessId) -> Option<ProcessView> {
        self.supervisor.view(id)
    }

    /// The process supervisor (C2) — start/stop/restart and bulk operations.
    pub fn supervisor(&self) -> &Supervisor {
        self.supervisor.as_ref()
    }

    /// Turns desktop notifications on or off globally — the single switch the notification
    /// reactor honours, so the UI, MCP, and CLI all toggle the same flag.
    pub fn set_notifications_enabled(&self, enabled: bool) {
        self.notifications_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Whether desktop notifications are currently enabled.
    pub fn notifications_enabled(&self) -> bool {
        self.notifications_enabled.load(Ordering::Relaxed)
    }

    /// Waits until process `id` is listening on `port`, or times out — port readiness (C5).
    /// While waiting the process reads Running-but-not-Ready ([`ProcessView::ready`] =
    /// `Readiness::Waiting`); on bind, `Readiness::Ready`. One method behind the Facade, so
    /// the MCP/HTTP/CLI callers share the behaviour.
    pub async fn wait_for_port(
        &self,
        id: ProcessId,
        port: u16,
        timeout: Duration,
    ) -> Result<(), WaitForPortError> {
        portscan::wait_for_port(
            self.supervisor.clone(),
            self.port_probe.clone(),
            self.clock.clone(),
            id,
            port,
            timeout,
        )
        .await
    }

    /// The project registry (C1).
    pub fn projects(&self) -> &Projects {
        &self.projects
    }

    /// The trust gate (C1).
    pub fn trust(&self) -> &TrustStore {
        &self.trust
    }

    /// The `solo.yml` sync engine (C1).
    pub fn config(&self) -> &ConfigEngine {
        &self.config
    }

    /// The agents context (C4): the agent-tool registry and `--version` auto-detection.
    pub fn agents(&self) -> &Agents {
        &self.agents
    }

    /// Opens a project end to end — see [`ProjectService::open`]. The Facade owns the
    /// contexts the lifecycle spans; it assembles the service and delegates, so the open
    /// sequence lives in the projects domain rather than being re-implemented here.
    pub fn load_project(&self, root: &Path) -> Result<ProjectLoad, LoadProjectError> {
        self.project_service().open(root)
    }

    /// Re-registers every known project without starting anything (session restore on
    /// launch) — see [`ProjectService::restore`]. Delegates to the projects domain.
    pub fn restore_projects(&self) {
        self.project_service().restore();
    }

    /// Assembles the project lifecycle service over the contexts the Facade owns.
    fn project_service(&self) -> ProjectService<'_> {
        ProjectService::new(&self.projects, &self.config, &self.supervisor, &self.bus)
    }

    /// The project read model: every known project's display identity. The snapshot
    /// half of snapshot-then-deltas — pair it with [`DomainEvent::ProjectOpened`].
    pub fn projects_snapshot(&self) -> Result<Vec<ProjectView>, StoreError> {
        self.projects.views()
    }

    /// Trusts a project's command by name: resolves the command to its current variant
    /// from the loaded `solo.yml`, records trust for that variant, and updates the read
    /// model so the command becomes startable. One method behind the trust gate, so the
    /// UI, MCP, and CLI grant trust identically. Untrusting is not yet exposed.
    pub fn trust_command(&self, project: ProjectId, name: &str) -> Result<(), TrustCommandError> {
        let spec = self
            .config
            .spec(project, name)
            .ok_or(TrustCommandError::NotFound)?;
        self.trust.trust(project, &spec)?;
        self.supervisor.mark_trusted(project, &spec.variant_hash());
        Ok(())
    }

    /// Launches a configured agent tool as an interactive **Agent** process in a project's
    /// directory and starts it. Resolves the tool from the registry and the project's
    /// working directory, composes the tool's command line with `extra_args` for this one
    /// launch ("agent with flags"), then registers and starts an ungated
    /// [`ProcessKind::Agent`] on the real PTY — never headless `-p` — so the CLI's own native
    /// login can run in the terminal pane. Many agents can run concurrently; each call is a
    /// new process.
    ///
    /// Soloist stores or injects **no** agent credential: the spawn carries no env
    /// overrides, so the agent inherits Soloist's environment unchanged — `$DISPLAY`/`$BROWSER`
    /// for a loopback-OAuth browser step and any `ANTHROPIC_*` the user set pass straight
    /// through, and the CLI keeps using whatever auth the user already configured.
    ///
    /// One method behind the Facade, so the UI launch picker now and the MCP `spawn_agent`
    /// tool later launch agents identically. Must run within a `tokio` runtime (starting
    /// spawns the actor).
    pub fn launch_agent(
        &self,
        project: ProjectId,
        tool: &str,
        extra_args: Vec<String>,
    ) -> Result<ProcessId, LaunchAgentError> {
        let tool = self
            .agents
            .tool(tool)?
            .ok_or(LaunchAgentError::UnknownTool)?;
        let root = self
            .projects
            .get(project)?
            .ok_or(LaunchAgentError::UnknownProject)?
            .root;
        let spec = SpawnSpec {
            command: tool.launch_command_line(&extra_args),
            working_dir: root,
            // No env overrides: the agent inherits Soloist's environment as-is so its own
            // native auth flow works untouched. Soloist injects no credential.
            env: BTreeMap::new(),
            size: PtySize::default(),
        };
        let kind = tool.kind;
        let id = self.supervisor.register(Registration::launched(
            project,
            ProcessKind::Agent,
            tool.name,
            spec,
        ));
        self.supervisor.start(id)?;
        // Track the agent's idle activity from now on; the idle sampler reclassifies it each
        // interval using its provider's heuristic.
        self.idle.track(id, kind);
        Ok(id)
    }
}

/// Why trusting a command failed: it is not in the loaded config, or the durable trust
/// write failed.
#[derive(Debug, thiserror::Error)]
pub enum TrustCommandError {
    #[error("no such command in the loaded project config")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why launching an agent failed: no tool is registered under that name, the project is not
/// known, a durable read failed, or the supervisor refused to start the process.
#[derive(Debug, thiserror::Error)]
pub enum LaunchAgentError {
    #[error("no agent tool registered under that name")]
    UnknownTool,
    #[error("no such project")]
    UnknownProject,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
}

#[cfg(test)]
#[path = "facade_tests.rs"]
mod tests;
