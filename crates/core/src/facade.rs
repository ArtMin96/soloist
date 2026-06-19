//! The public command and query API that adapters call (context C8).
//!
//! [`Facade`] is the one surface every adapter (Tauri, MCP, HTTP/CLI) talks to. It
//! owns the event bus and the bounded contexts — process supervision (C2), and the
//! projects/trust/config of C1 — and hands adapters references to them, so a behaviour
//! like "restart" or "is this command trusted" is implemented exactly once. Adapters
//! translate requests in and project the read model out; they hold no business state.

use std::path::Path;

use tokio::sync::broadcast;

use crate::config::{ConfigEngine, ConfigError};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::ports::{CorePorts, StoreError};
use crate::process::ProcessView;
use crate::projects::{ProjectError, ProjectView, Projects};
use crate::supervisor::{Registration, Supervisor, SupervisorError};
use crate::trust::TrustStore;

/// Per-subscriber event buffer. Bounded so a stalled adapter re-syncs from a snapshot
/// (see [`crate::events`]) rather than growing memory without limit.
const EVENT_BUFFER: usize = 1024;

/// The integration façade (context C8). Cheap to share as Tauri-managed state.
pub struct Facade {
    bus: EventBus,
    supervisor: Supervisor,
    projects: Projects,
    trust: TrustStore,
    config: ConfigEngine,
}

impl Facade {
    /// Builds a façade over the given core port set (real adapters in the app, fakes in
    /// tests). The trust repository is shared by the supervisor's trust gate, the trust
    /// store, and the config sync engine, so all three agree on what is trusted.
    pub fn new(ports: CorePorts) -> Self {
        let bus = EventBus::new(EVENT_BUFFER);
        let supervisor = Supervisor::new(&ports, bus.clone());
        let CorePorts {
            trust, projects, ..
        } = ports;
        Self {
            supervisor,
            projects: Projects::new(projects),
            trust: TrustStore::new(trust.clone()),
            config: ConfigEngine::new(trust, bus.clone()),
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
        &self.supervisor
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

    /// Opens a project: registers its root (assigning the durable [`ProjectId`]), loads
    /// its `solo.yml`, registers each declared process as a trust-gated command, then
    /// reconciles leftover process groups and starts the trusted auto-start commands.
    ///
    /// When the folder has no `solo.yml`, one is auto-created from its detected commands
    /// (A10) before opening, so a project opened from an arbitrary folder is usable; an
    /// existing `solo.yml` is never rewritten. Reconciliation runs **after** registration
    /// so a leftover group matching a `solo.yml` command is adopted rather than
    /// mis-surfaced as an orphan. Starting is the supervisor's trusted-auto-start subset,
    /// so a detected (hence untrusted) command is registered (visible, `Stopped`) but
    /// never run until its variant is trusted. Returns the project's id, how many
    /// processes its `solo.yml` declared, and whether the file was just created, so the
    /// caller can tell the user what happened instead of doing so silently. Must run
    /// within a `tokio` runtime (reconciliation and starting do).
    pub fn load_project(&self, root: &Path) -> Result<ProjectLoad, LoadProjectError> {
        let record = self.projects.add(root, None, None)?;
        let created = crate::config::create_if_absent(&record.root)?;
        let config = self.config.open(record.id, record.root.clone())?;
        // Persist the project's display metadata now the config is known. The id had to
        // be assigned first (`config.open` needs it), but the `name`/`icon` come from the
        // file — so a second idempotent upsert (keyed on the canonical root) records them.
        let record =
            self.projects
                .add(&record.root, config.name.as_deref(), config.icon.as_deref())?;
        // Announce the project before its processes, so an adapter folding deltas has the
        // project in its read model before any `ProcessSpawned` references it.
        let view = ProjectView::from_record(&record);
        self.bus.publish(DomainEvent::ProjectOpened {
            id: view.id,
            name: view.name.clone(),
            root: view.root.clone(),
            icon: view.icon.clone(),
        });
        for (name, spec) in &config.processes {
            self.supervisor
                .register(Registration::command(record.id, &record.root, name, spec));
        }
        self.supervisor.reconcile_orphans();
        self.supervisor.start_all(record.id)?;
        Ok(ProjectLoad {
            id: record.id,
            processes: config.processes.len(),
            created,
        })
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
}

/// The outcome of opening a project: its durable id, how many processes its `solo.yml`
/// declared, and whether that `solo.yml` was just auto-created from detected commands.
/// `created` lets the caller tell the user a config was made for them; `processes == 0`
/// with `created == false` means an existing `solo.yml` declared nothing — either way the
/// caller surfaces it so opening a project is never silent.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ProjectLoad {
    pub id: ProjectId,
    pub processes: usize,
    pub created: bool,
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

/// Why opening a project failed: resolving/persisting its root, reading its `solo.yml`,
/// or starting its trusted commands.
#[derive(Debug, thiserror::Error)]
pub enum LoadProjectError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Write(#[from] crate::config::WriteError),
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
    async fn load_project_registers_each_declared_command() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            crate::config::config_path(dir.path()),
            "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
        )
        .expect("write solo.yml");

        facade.load_project(dir.path()).expect("load");

        // Both commands are registered and resting; neither starts, because the config's
        // variants are untrusted (loading never bypasses the trust gate).
        let snapshot = facade.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert!(snapshot.iter().all(|p| p.status == ProcStatus::Stopped));
        let mut labels: Vec<_> = snapshot.iter().map(|p| p.label.clone()).collect();
        labels.sort();
        assert_eq!(labels, vec!["Api".to_string(), "Web".to_string()]);
    }

    #[tokio::test]
    async fn load_project_starts_a_trusted_auto_start_command() {
        let (facade, trust) = facade(FakeSpawner::exits_on_terminate());
        let mut rx = facade.subscribe();
        let dir = tempfile::tempdir().expect("temp dir");
        let yml = "processes:\n  Web:\n    command: npm run dev\n";
        std::fs::write(crate::config::config_path(dir.path()), yml).expect("write solo.yml");

        // Pre-register the project to learn its id and trust the command's variant, so
        // load's start_all reaches it (start is the trusted, auto-start subset; auto_start
        // defaults true).
        let record = facade
            .projects()
            .add(dir.path(), None, None)
            .expect("add project");
        let spec = crate::config::parse(yml)
            .expect("parse")
            .processes
            .get("Web")
            .cloned()
            .expect("Web");
        trust
            .set_trusted(record.id, &spec.variant_hash())
            .expect("trust");

        let project = facade.load_project(dir.path()).expect("load");
        assert_eq!(project.id, record.id);
        wait_for(&mut rx, ProcStatus::Running).await;
    }

    #[tokio::test]
    async fn load_project_reports_the_process_count() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());

        // A folder with no solo.yml loads successfully but declares nothing — the count
        // lets the caller tell the user instead of silently showing an unchanged screen.
        let empty = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            facade.load_project(empty.path()).expect("load").processes,
            0
        );

        // A folder whose solo.yml declares commands reports their number.
        let stack = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            crate::config::config_path(stack.path()),
            "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
        )
        .expect("write solo.yml");
        assert_eq!(
            facade.load_project(stack.path()).expect("load").processes,
            2
        );
    }

    #[tokio::test]
    async fn load_project_auto_creates_a_solo_yml_from_detected_commands() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"vite"}}"#,
        )
        .expect("write package.json");

        let load = facade.load_project(dir.path()).expect("load");

        // A solo.yml was created for the user and the detected command registered.
        assert!(load.created, "a solo.yml was auto-created");
        assert_eq!(load.processes, 1);
        assert!(crate::config::config_path(dir.path()).exists());
        let snapshot = facade.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].label, "dev");
        // Detected commands are untrusted — auto-create never bypasses the trust gate.
        assert!(snapshot[0].requires_trust);
    }

    #[tokio::test]
    async fn load_project_does_not_recreate_an_existing_solo_yml() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            crate::config::config_path(dir.path()),
            "processes:\n  Web:\n    command: npm run dev\n",
        )
        .expect("write solo.yml");

        let load = facade.load_project(dir.path()).expect("load");
        assert!(!load.created, "an existing solo.yml is not recreated");
        assert_eq!(load.processes, 1);
    }

    #[tokio::test]
    async fn load_project_persists_and_projects_the_display_name() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            crate::config::config_path(dir.path()),
            "name: Storefront\nprocesses:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
        )
        .expect("write solo.yml");

        let load = facade.load_project(dir.path()).expect("load");

        // The `solo.yml` name (previously dropped) is persisted and projected.
        let record = facade
            .projects()
            .get(load.id)
            .expect("get")
            .expect("record");
        assert_eq!(record.name.as_deref(), Some("Storefront"));
        let views = facade.projects_snapshot().expect("views");
        let view = views
            .iter()
            .find(|v| v.id == load.id)
            .expect("project view");
        assert_eq!(view.name, "Storefront");
    }

    #[tokio::test]
    async fn load_project_announces_the_opened_project() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let mut rx = facade.subscribe();
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            crate::config::config_path(dir.path()),
            "name: Storefront\nprocesses:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
        )
        .expect("write solo.yml");

        let load = facade.load_project(dir.path()).expect("load");

        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProjectOpened { id, name, .. }) => {
                    assert_eq!(id, load.id);
                    assert_eq!(name, "Storefront");
                    break;
                }
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("event bus closed before ProjectOpened"),
            }
        }
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
}
