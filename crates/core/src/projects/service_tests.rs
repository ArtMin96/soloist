use super::*;
use crate::composition::CorePorts;
use crate::ids::ProcessId;
use crate::ports::{ProjectRepo, TokioClock, TrustRepo};
use crate::process::ProcStatus;
use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};
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
    parts_with_repo(spawner, Arc::new(FakeProjectRepo::new()))
}

fn parts_with_repo(spawner: FakeSpawner, repo: Arc<dyn ProjectRepo>) -> Parts {
    let bus = EventBus::new(1024);
    let trust = Arc::new(FakeTrustRepo::new());
    let ports = CorePorts::builder(
        Arc::new(spawner),
        Arc::new(TokioClock),
        trust.clone(),
        repo.clone(),
    )
    .build();
    let supervisor = Supervisor::new(ports.supervisor_ports(), bus.clone());
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
async fn reopening_a_project_reconciles_commands_instead_of_duplicating() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(
        dir.path(),
        "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
    );

    let first = parts.service().open(dir.path()).expect("first open");
    let second = parts.service().open(dir.path()).expect("second open");

    // Re-opening the same folder (the picker on an open project, a forwarded second launch)
    // keeps the durable id and reconciles each command in place — never a second row per name.
    assert_eq!(first.id, second.id);
    let snapshot = parts.supervisor.snapshot();
    assert_eq!(snapshot.len(), 2);
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

/// Opens a project with one trusted, auto-starting `Web` command already running, returning
/// the parts, an event receiver, the project id, and `Web`'s process id — the setup the
/// running-command reload tests share.
async fn opened_running_web(
    parts: &Parts,
    rx: &mut broadcast::Receiver<DomainEvent>,
    dir: &Path,
) -> (ProjectId, ProcessId) {
    let yml = "processes:\n  Web:\n    command: npm run dev\n";
    write_yml(dir, yml);
    let record = parts.projects.add(dir, None, None).expect("add");
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
    let load = parts.service().open(dir).expect("open");
    wait_for(rx, ProcStatus::Running).await;
    (load.id, parts.supervisor.snapshot()[0].id)
}

#[tokio::test]
async fn reload_registers_an_added_command_resting() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(dir.path(), "processes:\n  Web:\n    command: npm run dev\n");
    let load = parts.service().open(dir.path()).expect("open");

    // A command added to solo.yml appears after a reload, registered resting (never started).
    write_yml(
        dir.path(),
        "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
    );
    let diff = parts
        .service()
        .reload(load.id)
        .expect("reload")
        .expect("a change");
    assert_eq!(diff.added, vec!["Api"]);
    let mut labels: Vec<_> = parts
        .supervisor
        .snapshot()
        .iter()
        .map(|p| p.label.clone())
        .collect();
    labels.sort();
    assert_eq!(labels, vec!["Api".to_string(), "Web".to_string()]);
    assert!(parts
        .supervisor
        .snapshot()
        .iter()
        .all(|p| p.status == ProcStatus::Stopped));
}

#[tokio::test]
async fn reload_updates_a_changed_spec_in_place_and_recomputes_trust() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    // `auto_start: false` so trusting the variant does not start it — this exercises the
    // resting-update path, and the variant covers command/dir/env (not auto_start).
    let yml = "processes:\n  Web:\n    command: npm run dev\n    auto_start: false\n";
    write_yml(dir.path(), yml);
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
    let web = parts.supervisor.snapshot()[0].id;
    assert!(!parts.supervisor.view(web).expect("view").requires_trust);

    // Changing the command updates the one registration in place — id stable, never
    // duplicated — and the new, untrusted variant flips `requires_trust`.
    write_yml(
        dir.path(),
        "processes:\n  Web:\n    command: npm run start\n    auto_start: false\n",
    );
    let diff = parts
        .service()
        .reload(load.id)
        .expect("reload")
        .expect("a change");
    assert_eq!(diff.updated, vec!["Web"]);
    let snapshot = parts.supervisor.snapshot();
    assert_eq!(
        snapshot.len(),
        1,
        "an updated command is replaced in place, never duplicated"
    );
    assert_eq!(snapshot[0].id, web, "the id is stable across a spec change");
    assert!(
        parts.supervisor.view(web).expect("view").requires_trust,
        "the new, untrusted variant re-flags trust"
    );
}

#[tokio::test]
async fn reload_leaves_a_running_command_untouched_when_its_spec_changes() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let mut rx = parts.bus.subscribe();
    let dir = tempfile::tempdir().expect("temp dir");
    let (project, web) = opened_running_web(&parts, &mut rx, dir.path()).await;

    // A spec change never kills running work: Web keeps running on its current launch until
    // its next restart, with the new spec stored beneath it.
    write_yml(
        dir.path(),
        "processes:\n  Web:\n    command: npm run start\n",
    );
    parts.service().reload(project).expect("reload");
    assert_eq!(
        parts.supervisor.view(web).expect("still registered").status,
        ProcStatus::Running,
        "reload never kills a running command"
    );
    assert_eq!(parts.supervisor.snapshot().len(), 1);
}

#[tokio::test]
async fn reload_drops_a_removed_resting_command() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(
        dir.path(),
        "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
    );
    let load = parts.service().open(dir.path()).expect("open");

    // Removing a resting command from solo.yml drops it on reload.
    write_yml(dir.path(), "processes:\n  Web:\n    command: npm run dev\n");
    let diff = parts
        .service()
        .reload(load.id)
        .expect("reload")
        .expect("a change");
    assert_eq!(diff.removed, vec!["Api"]);
    let labels: Vec<_> = parts
        .supervisor
        .snapshot()
        .iter()
        .map(|p| p.label.clone())
        .collect();
    assert_eq!(labels, vec!["Web".to_string()]);
}

#[tokio::test]
async fn reload_keeps_a_removed_running_command() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let mut rx = parts.bus.subscribe();
    let dir = tempfile::tempdir().expect("temp dir");
    let (project, web) = opened_running_web(&parts, &mut rx, dir.path()).await;

    // Removing a *running* command never kills it: it is left registered and running for the
    // user to stop explicitly. (A different command replaces it so the file is not empty.)
    write_yml(dir.path(), "processes:\n  Api:\n    command: cargo run\n");
    parts.service().reload(project).expect("reload");
    assert_eq!(
        parts
            .supervisor
            .view(web)
            .expect("running command kept")
            .status,
        ProcStatus::Running
    );
}

#[tokio::test]
async fn reload_applies_a_rename_in_place() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(dir.path(), "processes:\n  Web:\n    command: npm run dev\n");
    let load = parts.service().open(dir.path()).expect("open");
    let web = parts.supervisor.snapshot()[0].id;

    // Renaming the process (same command) relabels the one registration in place — same id,
    // no duplicate.
    write_yml(
        dir.path(),
        "processes:\n  Frontend:\n    command: npm run dev\n",
    );
    let diff = parts
        .service()
        .reload(load.id)
        .expect("reload")
        .expect("a change");
    assert_eq!(diff.renamed.len(), 1);
    let snapshot = parts.supervisor.snapshot();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].id, web, "a rename keeps the id");
    assert_eq!(snapshot[0].label, "Frontend");
}

#[tokio::test]
async fn reload_of_an_unchanged_file_is_a_noop() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(dir.path(), "processes:\n  Web:\n    command: npm run dev\n");
    let load = parts.service().open(dir.path()).expect("open");

    // A byte-identical file yields no diff — the reconcile does nothing.
    assert!(parts.service().reload(load.id).expect("reload").is_none());
    assert_eq!(parts.supervisor.snapshot().len(), 1);
}

#[tokio::test]
async fn reload_of_an_unknown_project_is_an_error() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    assert!(matches!(
        parts.service().reload(ProjectId::from_raw(9999)),
        Err(ReloadError::UnknownProject)
    ));
}

#[tokio::test]
async fn reload_reuses_the_registration_when_a_removed_running_command_is_readded() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let mut rx = parts.bus.subscribe();
    let dir = tempfile::tempdir().expect("temp dir");
    let (project, web) = opened_running_web(&parts, &mut rx, dir.path()).await;

    // Remove Web while it runs — a running command is kept, not dropped (so its registration
    // lingers under the same name).
    write_yml(dir.path(), "processes:\n  Api:\n    command: cargo run\n");
    parts
        .service()
        .reload(project)
        .expect("reload removes Web from config");

    // Re-add Web: the reconcile must apply it onto the still-running registration, not mint a
    // second one under the same name.
    write_yml(
        dir.path(),
        "processes:\n  Api:\n    command: cargo run\n  Web:\n    command: npm run dev\n",
    );
    parts.service().reload(project).expect("reload re-adds Web");

    let webs: Vec<_> = parts
        .supervisor
        .snapshot()
        .into_iter()
        .filter(|p| p.label == "Web")
        .collect();
    assert_eq!(
        webs.len(),
        1,
        "a re-added running command is not duplicated"
    );
    assert_eq!(webs[0].id, web, "it keeps the original registration's id");
    assert_eq!(webs[0].status, ProcStatus::Running, "and stays running");
}

#[tokio::test]
async fn remove_reaps_a_running_process_before_announcing_the_removal() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let mut rx = parts.bus.subscribe();
    let dir = tempfile::tempdir().expect("temp dir");
    let (project, web) = opened_running_web(&parts, &mut rx, dir.path()).await;

    parts.service().remove(project).await.expect("remove");

    // The running command was stopped, reaped, and forgotten — nothing stays registered.
    assert!(parts.supervisor.snapshot().is_empty());
    // The durable record is gone.
    assert!(parts.projects.get(project).expect("get").is_none());
    // The process's own removal is announced before the project's — a consumer never sees
    // a project vanish while its processes still look alive.
    let mut web_removed = false;
    loop {
        match rx.recv().await {
            Ok(DomainEvent::ProcessRemoved { id }) if id == web => web_removed = true,
            Ok(DomainEvent::ProjectRemoved { id }) => {
                assert_eq!(id, project);
                assert!(web_removed, "ProcessRemoved precedes ProjectRemoved");
                break;
            }
            Ok(_) | Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => panic!("event bus closed before ProjectRemoved"),
        }
    }
}

#[tokio::test]
async fn remove_drops_resting_registrations_and_evicts_sync_state() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(
        dir.path(),
        "processes:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
    );
    let load = parts.service().open(dir.path()).expect("open");
    assert_eq!(parts.supervisor.snapshot().len(), 1);

    parts.service().remove(load.id).await.expect("remove");

    // A never-started (resting) registration is forgotten too.
    assert!(parts.supervisor.snapshot().is_empty());
    // The engine no longer tracks the file: an edit after removal yields no diff.
    write_yml(dir.path(), "processes:\n  Api:\n    command: cargo run\n");
    assert!(parts.config.sync(load.id).expect("sync").is_none());
    // Removal never touches disk — the user's solo.yml remains.
    assert!(crate::config::config_path(dir.path()).exists());
}

/// A project repo that records which thread each durable call ran on, so a test can prove a call
/// did not run on a runtime worker. Every call otherwise passes straight through.
#[derive(Default)]
struct ThreadRecordingRepo {
    inner: FakeProjectRepo,
    read_thread: Mutex<Option<ThreadId>>,
    delete_thread: Mutex<Option<ThreadId>>,
}

impl ThreadRecordingRepo {
    fn read_thread(&self) -> Option<ThreadId> {
        *self.read_thread.lock().expect("uncontended")
    }

    fn delete_thread(&self) -> Option<ThreadId> {
        *self.delete_thread.lock().expect("uncontended")
    }
}

impl ProjectRepo for ThreadRecordingRepo {
    fn upsert(
        &self,
        root: &Path,
        name: Option<&str>,
        icon: Option<&Path>,
    ) -> Result<ProjectRecord, StoreError> {
        self.inner.upsert(root, name, icon)
    }

    fn list(&self) -> Result<Vec<ProjectRecord>, StoreError> {
        self.inner.list()
    }

    fn get(&self, id: ProjectId) -> Result<Option<ProjectRecord>, StoreError> {
        *self.read_thread.lock().expect("uncontended") = Some(thread::current().id());
        self.inner.get(id)
    }

    fn remove(&self, id: ProjectId) -> Result<(), StoreError> {
        *self.delete_thread.lock().expect("uncontended") = Some(thread::current().id());
        self.inner.remove(id)
    }
}

/// Removal is an async path, and both of its store calls are synchronous — so each must run off the
/// runtime worker. The cascading delete matters most: it is the widest durable write the app makes,
/// and running it inline would park a worker for the length of its `fsync` on a slow or full disk.
/// The single-threaded test runtime runs this body on its one worker, so that thread's id is the
/// worker's: a store call recording it is a call that ran on the runtime.
#[tokio::test]
async fn remove_runs_its_store_calls_off_the_runtime_worker() {
    let runtime_worker = thread::current().id();
    let repo = Arc::new(ThreadRecordingRepo::default());
    let parts = parts_with_repo(FakeSpawner::exits_on_terminate(), repo.clone());
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(
        dir.path(),
        "processes:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
    );
    let load = parts.service().open(dir.path()).expect("open");

    parts.service().remove(load.id).await.expect("remove");

    assert_ne!(
        repo.read_thread().expect("the existence read ran"),
        runtime_worker,
        "the existence read must run off the runtime worker"
    );
    assert_ne!(
        repo.delete_thread().expect("the cascading delete ran"),
        runtime_worker,
        "the cascading delete must run off the runtime worker"
    );
    assert!(parts.projects.get(load.id).expect("get").is_none());
}

#[tokio::test]
async fn remove_of_an_unknown_project_is_an_error_and_touches_nothing() {
    let parts = parts(FakeSpawner::exits_on_terminate());
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(
        dir.path(),
        "processes:\n  Web:\n    command: npm run dev\n    auto_start: false\n",
    );
    let load = parts.service().open(dir.path()).expect("open");

    assert!(matches!(
        parts.service().remove(ProjectId::from_raw(9999)).await,
        Err(RemoveProjectError::UnknownProject)
    ));

    // The open project is untouched: still registered, still listed.
    assert_eq!(parts.supervisor.snapshot().len(), 1);
    assert!(parts.projects.get(load.id).expect("get").is_some());
}
