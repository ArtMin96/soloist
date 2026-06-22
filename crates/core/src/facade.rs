//! The public command and query API that adapters call (context C8).
//!
//! [`Facade`] is the one surface every adapter (Tauri, MCP, HTTP/CLI) talks to. It
//! owns the event bus and the bounded contexts — process supervision (C2), and the
//! projects/trust/config of C1 — and hands adapters references to them, so a behaviour
//! like "restart" or "is this command trusted" is implemented exactly once. Adapters
//! translate requests in and project the read model out; they hold no business state.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use crate::agents::Agents;
use crate::config::ConfigEngine;
use crate::events::{DomainEvent, EventBus};
use crate::filewatch::{FileWatcher, WatchReactor};
use crate::ids::{ProcessId, ProjectId};
use crate::metrics::{MetricsProbe, MetricsSampler};
use crate::notify::{NotificationReactor, Notifier};
use crate::ports::{Clock, CorePorts, PtySize, SpawnSpec, StoreError};
use crate::portscan::{self, PortProbe, PortScanner, WaitForPortError};
use crate::process::{ProcessKind, ProcessView};
use crate::projects::{LoadProjectError, ProjectLoad, ProjectService, ProjectView, Projects};
use crate::supervisor::{Registration, Supervisor, SupervisorError};
use crate::trust::TrustStore;

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
            ..
        } = ports;
        Self {
            supervisor,
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

    /// The process supervisor (C2) — start/stop/restart and bulk operations.
    pub fn supervisor(&self) -> &Supervisor {
        self.supervisor.as_ref()
    }

    /// The self-healing reactor loop (crash auto-restart, C2), returned for the
    /// composition root to spawn once on its runtime. It runs until the facade is
    /// dropped; the supervisor's restart policy drives it.
    pub fn self_healing_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        self.supervisor.self_healing_loop()
    }

    /// The metrics sampler loop (monitoring C5), returned for the composition root to spawn
    /// once on its runtime. It samples each running process group on an interval and
    /// publishes a [`DomainEvent::MetricsTick`] per group, watching the supervisor weakly so
    /// it ends when the facade is dropped. Self-supervised: a panicking sample is isolated
    /// and the loop restarts. With the default [`crate::metrics::NoopMetricsProbe`] it emits
    /// nothing — the real CPU/memory adapter is chosen in the composition root.
    pub fn metrics_sampler_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        MetricsSampler::new(
            self.clock.clone(),
            self.metrics.clone(),
            self.bus.clone(),
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }

    /// The port-discovery scanner loop (monitoring C5), returned for the composition root to
    /// spawn once on its runtime. It discovers each running process group's listening ports,
    /// reflects them on [`ProcessView::ports`], and publishes [`DomainEvent::PortsChanged`]
    /// on a real change. Watches the supervisor weakly and is self-supervised, like the
    /// metrics sampler. With the default [`crate::portscan::NoopPortProbe`] it finds nothing.
    pub fn port_scanner_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        PortScanner::new(
            self.clock.clone(),
            self.port_probe.clone(),
            self.bus.clone(),
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }

    /// The file-watch reactor loop (monitoring C5), returned for the composition root to spawn
    /// once on its runtime. It watches each trusted, file-watched command's project root and,
    /// on a matching change, restarts that command (debounced) via the supervisor — reusing
    /// one restart behaviour. Watches the supervisor weakly and ends when the bus closes (app
    /// shutdown). With the default [`crate::filewatch::NoopFileWatcher`] it watches nothing,
    /// so the real `notify` adapter is chosen in the composition root.
    pub fn file_watch_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        WatchReactor::new(
            self.clock.clone(),
            self.file_watcher.clone(),
            &self.bus,
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }

    /// The notification reactor loop (notifications C7), returned for the composition root to
    /// spawn once on its runtime. It shows a desktop toast on a crash or an exhausted
    /// auto-restart (honouring the global on/off), watching the supervisor weakly so it ends
    /// when the facade is dropped. With the default [`crate::notify::NoopNotifier`] it shows
    /// nothing — the real desktop adapter is chosen in the composition root.
    pub fn notifications_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        NotificationReactor::new(
            self.notifier.clone(),
            self.notifications_enabled.clone(),
            &self.bus,
            Arc::downgrade(&self.supervisor),
        )
        .run()
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
    /// directory and starts it (E4). Resolves the tool from the registry and the project's
    /// working directory, composes the tool's command line with `extra_args` for this one
    /// launch ("agent with flags"), then registers and starts an ungated
    /// [`ProcessKind::Agent`] on the real PTY — never headless `-p` — so the CLI's own native
    /// login can run in the terminal pane. Many agents can run concurrently; each call is a
    /// new process.
    ///
    /// Soloist stores or injects **no** agent credential (E8): the spawn carries no env
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
            // native auth flow works untouched (E8). Soloist injects no credential.
            env: BTreeMap::new(),
            size: PtySize::default(),
        };
        let id = self.supervisor.register(Registration::launched(
            project,
            ProcessKind::Agent,
            tool.name,
            spec,
        ));
        self.supervisor.start(id)?;
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
mod tests {
    use super::*;
    use crate::ids::ProjectId;
    use crate::ports::{TokioClock, TrustRepo};
    use crate::process::ProcStatus;
    use crate::supervisor::{Registration, SupervisorError};
    use crate::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
    use std::path::Path;
    use std::sync::Arc;
    use tokio::sync::broadcast::error::RecvError;

    fn facade(spawner: FakeSpawner) -> (Facade, Arc<FakeTrustRepo>) {
        let trust = Arc::new(FakeTrustRepo::new());
        let facade = Facade::new(
            CorePorts::builder(
                Arc::new(spawner),
                Arc::new(TokioClock),
                trust.clone(),
                Arc::new(FakeProjectRepo::new()),
            )
            .build(),
        );
        (facade, trust)
    }

    async fn wait_for(rx: &mut broadcast::Receiver<DomainEvent>, target: ProcStatus) {
        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProcessStatusChanged { to, .. }) if to == target => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    }

    #[tokio::test]
    async fn the_facade_registers_starts_and_stops_a_process() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let mut rx = facade.subscribe();

        let id = facade.supervisor().register(terminal_registration(
            ProjectId::from_raw(1),
            "term",
            "sleep 60",
        ));
        // Starting an ungated terminal cannot fail the trust gate.
        facade
            .supervisor()
            .start(id)
            .expect("ungated terminal starts");
        assert_eq!(facade.snapshot().len(), 1);
        wait_for(&mut rx, ProcStatus::Running).await;

        // Stop routes through the same supervisor the snapshot reflects.
        assert!(facade.supervisor().stop(id));
        wait_for(&mut rx, ProcStatus::Stopped).await;
    }

    #[tokio::test]
    async fn the_trust_gate_is_enforced_through_the_facade() {
        let (facade, trust) = facade(FakeSpawner::exits_on_terminate());
        let config =
            crate::config::parse("processes:\n  Web:\n    command: npm run dev\n").expect("parse");
        let spec = config.processes.get("Web").cloned().expect("Web");
        let project = ProjectId::from_raw(1);
        let id = facade.supervisor().register(Registration::command(
            project,
            Path::new("/p"),
            "Web",
            &spec,
        ));

        assert!(matches!(
            facade.supervisor().start(id),
            Err(SupervisorError::Untrusted)
        ));

        trust
            .set_trusted(project, &spec.variant_hash())
            .expect("trust");
        facade.supervisor().start(id).expect("start once trusted");
    }

    #[tokio::test]
    async fn the_facade_exposes_the_agent_registry_and_detection() {
        use crate::agents::AgentTool;
        use crate::testing::{FakeAgentToolRepo, FakeVersionProbe};

        let tools = AgentTool::builtin_defaults();
        let facade = Facade::new(
            CorePorts::builder(
                Arc::new(FakeSpawner::exits_on_terminate()),
                Arc::new(TokioClock),
                Arc::new(FakeTrustRepo::new()),
                Arc::new(FakeProjectRepo::new()),
            )
            .agent_tools(Arc::new(FakeAgentToolRepo::new(tools.clone())))
            .version_probe(Arc::new(FakeVersionProbe::new(&["claude"])))
            .build(),
        );

        // The registry and auto-detection both route through the one agents context.
        assert_eq!(facade.agents().list_tools().expect("list"), tools);
        let detected = facade.agents().detect_installed().await.expect("detect");
        let claude = detected
            .iter()
            .find(|d| d.tool.command == "claude")
            .expect("claude detected");
        assert!(claude.installed, "the probed CLI is reported installed");
    }

    #[tokio::test]
    async fn trust_command_makes_an_untrusted_command_startable() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            crate::config::config_path(dir.path()),
            "processes:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
        )
        .expect("write solo.yml");
        let project = facade.load_project(dir.path()).expect("load");

        // Registered untrusted: the read model flags it and the gate refuses to start it.
        let web = || {
            facade
                .snapshot()
                .into_iter()
                .find(|p| p.label == "Web")
                .expect("Web")
        };
        assert!(web().requires_trust);
        assert!(matches!(
            facade.supervisor().start(web().id),
            Err(SupervisorError::Untrusted)
        ));

        facade
            .trust_command(project.id, "Web")
            .expect("trust the command");

        // The flag clears and the same start path now succeeds.
        assert!(!web().requires_trust);
        facade
            .supervisor()
            .start(web().id)
            .expect("starts once trusted");
    }

    #[tokio::test]
    async fn trust_command_rejects_an_unknown_command() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            crate::config::config_path(dir.path()),
            "processes:\n  Web:\n    command: npm run dev\n",
        )
        .expect("write solo.yml");
        let project = facade.load_project(dir.path()).expect("load");

        assert!(matches!(
            facade.trust_command(project.id, "Missing"),
            Err(TrustCommandError::NotFound)
        ));
    }

    /// A façade seeded with the built-in agent tools (so `"Claude"` resolves) over an
    /// in-memory project repo, for the agent-launch path.
    fn facade_with_tools(spawner: FakeSpawner) -> Facade {
        use crate::agents::AgentTool;
        use crate::testing::FakeAgentToolRepo;
        Facade::new(
            CorePorts::builder(
                Arc::new(spawner),
                Arc::new(TokioClock),
                Arc::new(FakeTrustRepo::new()),
                Arc::new(FakeProjectRepo::new()),
            )
            .agent_tools(Arc::new(FakeAgentToolRepo::new(
                AgentTool::builtin_defaults(),
            )))
            .build(),
        )
    }

    #[tokio::test]
    async fn launch_agent_registers_and_starts_an_agent_in_the_project() {
        use crate::process::ProcessKind;

        let facade = facade_with_tools(FakeSpawner::exits_on_terminate());
        let mut rx = facade.subscribe();
        let dir = tempfile::tempdir().expect("temp dir");
        let project = facade.load_project(dir.path()).expect("load");

        let id = facade
            .launch_agent(project.id, "Claude", Vec::new())
            .expect("launch");

        // It appears as an ungated Agent-kind process labelled by the tool, and starts.
        let view = facade
            .snapshot()
            .into_iter()
            .find(|p| p.id == id)
            .expect("launched agent in snapshot");
        assert_eq!(view.kind, ProcessKind::Agent);
        assert_eq!(view.label, "Claude");
        assert!(
            !view.requires_trust,
            "a launched agent is never trust-gated"
        );
        wait_for(&mut rx, ProcStatus::Running).await;
    }

    #[tokio::test]
    async fn launch_agent_rejects_an_unknown_tool() {
        let facade = facade_with_tools(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        let project = facade.load_project(dir.path()).expect("load");

        assert!(matches!(
            facade.launch_agent(project.id, "Nonexistent", Vec::new()),
            Err(LaunchAgentError::UnknownTool)
        ));
    }

    #[tokio::test]
    async fn launch_agent_rejects_an_unknown_project() {
        let facade = facade_with_tools(FakeSpawner::exits_on_terminate());

        assert!(matches!(
            facade.launch_agent(ProjectId::from_raw(9999), "Claude", Vec::new()),
            Err(LaunchAgentError::UnknownProject)
        ));
    }
}
