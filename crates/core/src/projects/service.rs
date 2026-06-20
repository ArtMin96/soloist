//! The project lifecycle: opening and restoring projects.
//!
//! [`ProjectService`] is the one place the "open a project" behaviour lives. It
//! orchestrates the contexts a project open touches — the registry (durable identity),
//! the `solo.yml` config engine, the supervisor (command registration, orphan
//! reconciliation, auto-start), and the event bus — so no consumer (the [`Facade`], an
//! adapter, the UI) re-implements that sequence. Consumers call [`ProjectService::open`]
//! or [`ProjectService::restore`]; they do not decide how a project opens.
//!
//! [`Facade`]: crate::facade::Facade

use std::path::Path;

use crate::config::{ConfigEngine, ConfigError, SoloYml, WriteError};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::ports::ProjectRecord;
use crate::projects::{ProjectError, Projects};
use crate::supervisor::{Registration, Supervisor, SupervisorError};

/// The project lifecycle service. Borrows the contexts a project open spans — assembled
/// once by the composition root (the [`Facade`](crate::facade::Facade) owns them) — so the
/// orchestration is defined here and nowhere else.
pub struct ProjectService<'a> {
    projects: &'a Projects,
    config: &'a ConfigEngine,
    supervisor: &'a Supervisor,
    bus: &'a EventBus,
}

impl<'a> ProjectService<'a> {
    /// Assembles the service over the contexts it orchestrates.
    pub fn new(
        projects: &'a Projects,
        config: &'a ConfigEngine,
        supervisor: &'a Supervisor,
        bus: &'a EventBus,
    ) -> Self {
        Self {
            projects,
            config,
            supervisor,
            bus,
        }
    }

    /// Opens a project: registers its root (assigning the durable [`ProjectId`]), loads
    /// its `solo.yml`, registers each declared process as a trust-gated command, then
    /// reconciles leftover process groups and starts the trusted auto-start commands.
    ///
    /// When the folder has no `solo.yml`, one is auto-created from its detected commands
    /// before opening, so a project opened from an arbitrary folder is usable; an existing
    /// `solo.yml` is never rewritten. Reconciliation runs **after** registration so a
    /// leftover group matching a `solo.yml` command is adopted rather than mis-surfaced as
    /// an orphan. Starting is the supervisor's trusted-auto-start subset, so a detected
    /// (hence untrusted) command is registered (visible, `Stopped`) but never run until its
    /// variant is trusted. Returns the project's id, how many processes its `solo.yml`
    /// declared, and whether the file was just created, so the caller can tell the user
    /// what happened instead of doing so silently. Must run within a `tokio` runtime
    /// (reconciliation and starting do).
    pub fn open(&self, root: &Path) -> Result<ProjectLoad, LoadProjectError> {
        let (record, config, created) = self.open_and_register(root)?;
        self.supervisor.reconcile_orphans();
        self.supervisor.start_all(record.id)?;
        Ok(ProjectLoad {
            id: record.id,
            processes: config.processes.len(),
            created,
        })
    }

    /// Re-registers every known project without starting anything (session restore on
    /// launch), then reconciles orphans once. Each project's commands reappear **resting**,
    /// so the sidebar shows your projects across runs while nothing is spawned on startup —
    /// starting stays an explicit action. Best-effort: a project whose root or `solo.yml`
    /// is no longer readable is skipped, never failing the launch. Must run within a
    /// `tokio` runtime (reconciliation may adopt a leftover group).
    pub fn restore(&self) {
        let Ok(projects) = self.projects.list() else {
            return;
        };
        for project in projects {
            let _ = self.open_and_register(&project.root);
        }
        self.supervisor.reconcile_orphans();
    }

    /// Adds the project (auto-creating its `solo.yml` when absent), loads the config,
    /// persists the resolved display metadata, announces the open, and registers each
    /// command as a trust-gated process — the shared path under [`Self::open`] (which then
    /// reconciles and starts) and [`Self::restore`] (which does neither). Returns the
    /// durable record, the parsed config, and whether the `solo.yml` was just created. Does
    /// not reconcile orphans or start — the caller decides.
    fn open_and_register(
        &self,
        root: &Path,
    ) -> Result<(ProjectRecord, SoloYml, bool), LoadProjectError> {
        let record = self.projects.add(root, None, None)?;
        let created = crate::config::create_if_absent(&record.root)?;
        let config = self.config.open(record.id, record.root.clone())?;
        // Persist the project's display metadata now the config is known. The id had to
        // be assigned first (`config.open` needs it), but the `name`/`icon` come from the
        // file — so a second idempotent upsert (keyed on the canonical root) records them.
        let record =
            self.projects
                .add(&record.root, config.name.as_deref(), config.icon.as_deref())?;
        // Announce the project before its processes, so an adapter re-reading the project
        // read model has it in view before any `ProcessSpawned` references it.
        self.bus
            .publish(DomainEvent::ProjectOpened { id: record.id });
        for (name, spec) in &config.processes {
            self.supervisor
                .register(Registration::command(record.id, &record.root, name, spec));
        }
        Ok((record, config, created))
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

/// Why opening a project failed: resolving/persisting its root, reading its `solo.yml`,
/// auto-creating one, or starting its trusted commands.
#[derive(Debug, thiserror::Error)]
pub enum LoadProjectError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Write(#[from] WriteError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{CorePorts, TokioClock, TrustRepo};
    use crate::process::ProcStatus;
    use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo};
    use std::sync::Arc;
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::RecvError;

    /// The contexts a [`ProjectService`] orchestrates, wired to share one trust and one
    /// project repository — mirroring how the composition root assembles them.
    struct Parts {
        projects: Projects,
        config: ConfigEngine,
        supervisor: Supervisor,
        bus: EventBus,
        trust: Arc<FakeTrustRepo>,
    }

    impl Parts {
        fn service(&self) -> ProjectService<'_> {
            ProjectService::new(&self.projects, &self.config, &self.supervisor, &self.bus)
        }
    }

    fn parts(spawner: FakeSpawner) -> Parts {
        let bus = EventBus::new(1024);
        let trust = Arc::new(FakeTrustRepo::new());
        let repo = Arc::new(FakeProjectRepo::new());
        let ports = CorePorts::builder(
            Arc::new(spawner),
            Arc::new(TokioClock),
            trust.clone(),
            repo.clone(),
        )
        .build();
        let supervisor = Supervisor::new(&ports, bus.clone());
        let config = ConfigEngine::new(trust.clone(), bus.clone());
        let projects = Projects::new(repo);
        Parts {
            projects,
            config,
            supervisor,
            bus,
            trust,
        }
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

    fn write_yml(dir: &Path, yml: &str) {
        std::fs::write(crate::config::config_path(dir), yml).expect("write solo.yml");
    }

    #[tokio::test]
    async fn open_registers_each_declared_command() {
        let parts = parts(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        write_yml(
            dir.path(),
            "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
        );

        parts.service().open(dir.path()).expect("open");

        // Both commands are registered and resting; neither starts, because the config's
        // variants are untrusted (opening never bypasses the trust gate).
        let snapshot = parts.supervisor.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert!(snapshot.iter().all(|p| p.status == ProcStatus::Stopped));
        let mut labels: Vec<_> = snapshot.iter().map(|p| p.label.clone()).collect();
        labels.sort();
        assert_eq!(labels, vec!["Api".to_string(), "Web".to_string()]);
    }

    #[tokio::test]
    async fn open_starts_a_trusted_auto_start_command() {
        let parts = parts(FakeSpawner::exits_on_terminate());
        let mut rx = parts.bus.subscribe();
        let dir = tempfile::tempdir().expect("temp dir");
        let yml = "processes:\n  Web:\n    command: npm run dev\n";
        write_yml(dir.path(), yml);

        // Pre-register the project to learn its id and trust the command's variant, so
        // open's start_all reaches it (start is the trusted, auto-start subset; auto_start
        // defaults true).
        let record = parts.projects.add(dir.path(), None, None).expect("add");
        let spec = crate::config::parse(yml)
            .expect("parse")
            .processes
            .get("Web")
            .cloned()
            .expect("Web");
        parts
            .trust
            .set_trusted(record.id, &spec.variant_hash())
            .expect("trust");

        let load = parts.service().open(dir.path()).expect("open");
        assert_eq!(load.id, record.id);
        wait_for(&mut rx, ProcStatus::Running).await;
    }

    #[tokio::test]
    async fn open_reports_the_process_count() {
        let parts = parts(FakeSpawner::exits_on_terminate());

        // A folder with no solo.yml opens successfully but declares nothing — the count
        // lets the caller tell the user instead of silently showing an unchanged screen.
        let empty = tempfile::tempdir().expect("temp dir");
        assert_eq!(
            parts.service().open(empty.path()).expect("open").processes,
            0
        );

        // A folder whose solo.yml declares commands reports their number.
        let stack = tempfile::tempdir().expect("temp dir");
        write_yml(
            stack.path(),
            "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
        );
        assert_eq!(
            parts.service().open(stack.path()).expect("open").processes,
            2
        );
    }

    #[tokio::test]
    async fn open_auto_creates_a_solo_yml_from_detected_commands() {
        let parts = parts(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"vite"}}"#,
        )
        .expect("write package.json");

        let load = parts.service().open(dir.path()).expect("open");

        // A solo.yml was created for the user and the detected command registered.
        assert!(load.created, "a solo.yml was auto-created");
        assert_eq!(load.processes, 1);
        assert!(crate::config::config_path(dir.path()).exists());
        let snapshot = parts.supervisor.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].label, "dev");
        // Detected commands are untrusted — auto-create never bypasses the trust gate.
        assert!(snapshot[0].requires_trust);
    }

    #[tokio::test]
    async fn open_does_not_recreate_an_existing_solo_yml() {
        let parts = parts(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        write_yml(dir.path(), "processes:\n  Web:\n    command: npm run dev\n");

        let load = parts.service().open(dir.path()).expect("open");
        assert!(!load.created, "an existing solo.yml is not recreated");
        assert_eq!(load.processes, 1);
    }

    #[tokio::test]
    async fn open_persists_and_projects_the_display_name() {
        let parts = parts(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        write_yml(
            dir.path(),
            "name: Storefront\nprocesses:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
        );

        let load = parts.service().open(dir.path()).expect("open");

        // The `solo.yml` name (previously dropped) is persisted and projected.
        let record = parts.projects.get(load.id).expect("get").expect("record");
        assert_eq!(record.name.as_deref(), Some("Storefront"));
        let views = parts.projects.views().expect("views");
        let view = views.iter().find(|v| v.id == load.id).expect("view");
        assert_eq!(view.name, "Storefront");
    }

    #[tokio::test]
    async fn open_announces_the_opened_project() {
        let parts = parts(FakeSpawner::exits_on_terminate());
        let mut rx = parts.bus.subscribe();
        let dir = tempfile::tempdir().expect("temp dir");
        write_yml(
            dir.path(),
            "name: Storefront\nprocesses:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
        );

        let load = parts.service().open(dir.path()).expect("open");

        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProjectOpened { id }) => {
                    assert_eq!(id, load.id);
                    break;
                }
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("event bus closed before ProjectOpened"),
            }
        }
    }

    #[tokio::test]
    async fn restore_registers_known_projects_without_starting_them() {
        let parts = parts(FakeSpawner::exits_on_terminate());
        let dir = tempfile::tempdir().expect("temp dir");
        let yml = "processes:\n  Web:\n    command: npm run dev\n";
        write_yml(dir.path(), yml);

        // The project is durably known (as if opened in a prior run) and its auto-start
        // command is trusted — so `open` *would* start it. Restore must not.
        let record = parts.projects.add(dir.path(), None, None).expect("add");
        let spec = crate::config::parse(yml)
            .expect("parse")
            .processes
            .get("Web")
            .cloned()
            .expect("Web");
        parts
            .trust
            .set_trusted(record.id, &spec.variant_hash())
            .expect("trust");

        parts.service().restore();

        // The command reappears resting; restore registers but never spawns on launch.
        let snapshot = parts.supervisor.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].label, "Web");
        assert_eq!(snapshot[0].status, ProcStatus::Stopped);
    }
}
