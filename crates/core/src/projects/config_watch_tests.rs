//! Behavioural tests for [`ConfigWatchReactor`], kept out of the implementation file. They
//! drive a real [`ConfigEngine`] and [`Supervisor`] over fakes, feeding synthetic changed paths
//! straight onto a broadcast channel — the same shape [`crate::watchset::ProjectWatchSet`]'s
//! fan-out gives the reactor in production — against a real `solo.yml` on disk, so the hash-diff
//! path is the production one. Waits are event-driven and the debounce window is advanced on
//! the mock clock, so there is no OS watcher and no reliance on real time.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::broadcast;

use crate::composition::CorePorts;
use crate::config::{config_path, ConfigEngine};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::ports::Clock;
use crate::process::ProcStatus;
use crate::supervisor::Supervisor;
use crate::testing::{next_matching, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock};

use super::{ConfigWatchReactor, ProjectService, Projects, QUIET};

/// One advance step, comfortably past the reactor's quiet window so a single step fires it.
const STEP: Duration = Duration::from_millis(400);
/// How many changed paths the test's fan-out buffers before a send starts dropping — generous
/// for a single test's traffic.
const CHANGE_BUFFER: usize = 64;
/// How long one [`Setup::config_changed_until_armed`] attempt waits before resending — short,
/// since a resync that is going to catch up does so within a scheduler tick.
const RETRY_STEP: Duration = Duration::from_millis(50);
/// How many times [`Setup::config_changed_until_armed`] resends before giving up.
const RETRY_LIMIT: usize = 20;

struct Setup {
    projects: Arc<Projects>,
    config: Arc<ConfigEngine>,
    sup: Arc<Supervisor>,
    bus: EventBus,
    rx: broadcast::Receiver<DomainEvent>,
    clock: MockClock,
    changes: broadcast::Sender<PathBuf>,
}

impl Setup {
    fn service(&self) -> ProjectService<'_> {
        ProjectService::new(&self.projects, &self.config, &self.sup, &self.bus)
    }

    /// Opens the project rooted at `dir` and returns its id.
    fn open(&self, dir: &Path) -> ProjectId {
        self.service().open(dir).expect("open project").id
    }

    /// Sends `path` on the change fan-out, retrying until the reactor arms a reload debounce
    /// for it — bounded, so a resync that never catches up fails the test loudly rather than
    /// hanging. A live resync (on `ProjectOpened`) runs concurrently with whatever the test does
    /// next: a real filesystem watch cannot report a change before the directory it lives under
    /// is registered, but a test feeding a synthetic change straight onto the fan-out can race
    /// ahead of it, in which case the change simply matches nothing and has to be sent again.
    async fn config_changed_until_armed(&self, path: &Path) {
        let deadline = self.clock.now() + QUIET;
        for attempt in 0..RETRY_LIMIT {
            let _ = self.changes.send(path.to_path_buf());
            if tokio::time::timeout(RETRY_STEP, self.clock.deadline_armed_at(deadline))
                .await
                .is_ok()
            {
                return;
            }
            assert!(
                attempt + 1 < RETRY_LIMIT,
                "the reactor never armed a reload for {}",
                path.display(),
            );
        }
    }

    /// Feeds a burst of synthetic change events for the project's `solo.yml`, as an editor
    /// save would produce. Canonicalized, because the OS watcher reports real paths under
    /// the canonical root the registry stores. The first send retries until the reactor's
    /// resync has caught up; the rest of the burst is what coalescing is then asserted against.
    async fn config_changed_burst(&self, dir: &Path) {
        let root = dir.canonicalize().expect("canonical root");
        let path = config_path(&root);
        self.config_changed_until_armed(&path).await;
        for _ in 0..4 {
            let _ = self.changes.send(path.clone());
        }
    }
}

fn setup() -> Setup {
    let bus = EventBus::new(1024);
    let rx = bus.subscribe();
    let clock = MockClock::new();
    let trust = Arc::new(FakeTrustRepo::new());
    let repo = Arc::new(FakeProjectRepo::new());
    let ports = CorePorts::builder(
        Arc::new(FakeSpawner::exits_on_terminate()),
        Arc::new(clock.clone()),
        trust.clone(),
        repo.clone(),
    )
    .build();
    let sup = Arc::new(Supervisor::new(ports.supervisor_ports(), bus.clone()));
    let (changes, _rx) = broadcast::channel(CHANGE_BUFFER);
    Setup {
        projects: Arc::new(Projects::new(repo)),
        config: Arc::new(ConfigEngine::new(trust, bus.clone())),
        sup,
        bus,
        rx,
        clock,
        changes,
    }
}

fn write_yml(dir: &Path, yml: &str) {
    std::fs::write(config_path(dir), yml).expect("write solo.yml");
}

fn project_dir(yml: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    write_yml(dir.path(), yml);
    dir
}

async fn yield_many() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

/// Spawns the reactor over the test's own change fan-out.
fn spawn_reactor(s: &Setup) {
    tokio::spawn(
        ConfigWatchReactor::new(
            Arc::new(s.clock.clone()),
            s.changes.subscribe(),
            &s.bus,
            Arc::downgrade(&s.sup),
            s.projects.clone(),
            s.config.clone(),
        )
        .run(),
    );
}

/// Fires the debounce window and awaits the resulting `ConfigChanged` — the reload the
/// watched edit was debounced into.
async fn next_config_changed(s: &mut Setup) -> DomainEvent {
    s.clock.advance(STEP);
    next_matching(&mut s.rx, |e| {
        matches!(e, DomainEvent::ConfigChanged { .. })
    })
    .await
}

/// Fires several debounce windows and asserts no `ConfigChanged` was announced — the change
/// was a no-op, invalid, or not a config file.
async fn assert_no_config_changed(s: &mut Setup) {
    for _ in 0..5 {
        s.clock.advance(STEP);
        yield_many().await;
    }
    while let Ok(event) = s.rx.try_recv() {
        assert!(
            !matches!(event, DomainEvent::ConfigChanged { .. }),
            "unexpected ConfigChanged: {event:?}",
        );
    }
}

#[tokio::test]
async fn an_external_edit_burst_reloads_once_and_raises_the_trust_review() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    let project = s.open(dir.path());
    spawn_reactor(&s);

    // An external editor changes the command and saves — several FS events, one edit.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: echo two\n");
    s.config_changed_burst(dir.path()).await;
    yield_many().await;

    let DomainEvent::ConfigChanged {
        project: changed,
        diff,
        requires_trust,
        commands,
    } = next_config_changed(&mut s).await
    else {
        unreachable!()
    };
    assert_eq!(changed, project);
    assert_eq!(diff.updated, vec!["Echo".to_string()]);
    assert!(
        requires_trust,
        "an externally changed command variant needs re-trust"
    );
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "echo two");

    // The burst coalesced into exactly one reload, and the command was reconciled in
    // place — one row, still resting.
    assert_no_config_changed(&mut s).await;
    let snapshot = s.sup.snapshot();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].status, ProcStatus::Stopped);
}

#[tokio::test]
async fn an_added_command_is_registered_resting_even_with_auto_start() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    s.open(dir.path());
    spawn_reactor(&s);

    // The added command asks for auto-start — sync updates config only, and the variant is
    // untrusted, so it must appear resting, never running.
    write_yml(
        dir.path(),
        "processes:\n  Echo:\n    command: echo one\n  Extra:\n    command: sleep 5\n    auto_start: true\n",
    );
    s.config_changed_burst(dir.path()).await;
    yield_many().await;

    let DomainEvent::ConfigChanged { diff, .. } = next_config_changed(&mut s).await else {
        unreachable!()
    };
    assert_eq!(diff.added, vec!["Extra".to_string()]);
    let snapshot = s.sup.snapshot();
    assert_eq!(snapshot.len(), 2);
    assert!(
        snapshot.iter().all(|p| p.status == ProcStatus::Stopped),
        "a reload never starts anything: {snapshot:?}",
    );
}

#[tokio::test]
async fn a_byte_identical_rewrite_is_a_no_op() {
    let mut s = setup();
    let yml = "processes:\n  Echo:\n    command: echo one\n";
    let dir = project_dir(yml);
    s.open(dir.path());
    spawn_reactor(&s);

    // A tool re-saves the file without changing it: events fire, the hash matches.
    write_yml(dir.path(), yml);
    s.config_changed_burst(dir.path()).await;
    yield_many().await;

    assert_no_config_changed(&mut s).await;
}

#[tokio::test]
async fn the_apps_own_write_does_not_reload_again() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    let project = s.open(dir.path());
    spawn_reactor(&s);

    // The app edits its own config (a settings-pane save): `write` announces the change
    // itself and refreshes the sync hash to the written bytes.
    s.config
        .write(project, |config| {
            if let Some(spec) = config.processes.get_mut("Echo") {
                spec.command = "echo two".into();
            }
            Ok(())
        })
        .expect("write config");
    next_matching(&mut s.rx, |e| {
        matches!(e, DomainEvent::ConfigChanged { .. })
    })
    .await;

    // The OS watcher still reports our own write — the debounced re-read must hash equal
    // and announce nothing new.
    s.config_changed_burst(dir.path()).await;
    yield_many().await;

    assert_no_config_changed(&mut s).await;
}

#[tokio::test]
async fn an_invalid_edit_is_ignored_until_the_file_is_valid_again() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    s.open(dir.path());
    spawn_reactor(&s);

    // A mid-edit save is broken YAML: the reload fails quietly, the config keeps its last
    // good state, and the reactor stays alive.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: [broken\n");
    s.config_changed_burst(dir.path()).await;
    yield_many().await;
    assert_no_config_changed(&mut s).await;

    // The next valid save syncs normally — the failure was transient, not sticky.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: echo two\n");
    s.config_changed_burst(dir.path()).await;
    yield_many().await;

    let DomainEvent::ConfigChanged { diff, .. } = next_config_changed(&mut s).await else {
        unreachable!()
    };
    assert_eq!(diff.updated, vec!["Echo".to_string()]);
}

#[tokio::test]
async fn a_project_opened_after_startup_is_watched() {
    let mut s = setup();
    // Nothing open at startup — proven, not assumed: the initial resync must have actually run
    // against the empty registry before a project is opened, or the assertion below would prove
    // nothing about the *live* resync this test exists to check.
    spawn_reactor(&s);
    let _ = s.changes.send(PathBuf::from("/nonexistent/solo.yml"));
    assert_no_config_changed(&mut s).await;

    // Opening a project announces it; the reactor's live resync rebuilds its config-path
    // index to include it.
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    s.open(dir.path());

    // An external edit to the newly-opened project's config now reloads, once the resync has
    // caught up — proving the index is live, not only built at startup.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: echo two\n");
    s.config_changed_burst(dir.path()).await;
    yield_many().await;
    let DomainEvent::ConfigChanged { diff, .. } = next_config_changed(&mut s).await else {
        unreachable!()
    };
    assert_eq!(diff.updated, vec!["Echo".to_string()]);
}

#[tokio::test]
async fn a_removed_projects_config_is_not_reloaded() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    let project = s.open(dir.path());
    spawn_reactor(&s);

    // Removing the project announces it; whether the resync has already dropped it from the
    // config-path index or a reload is attempted and finds no such project, either way a late
    // edit for its `solo.yml` is not a reload.
    s.service().remove(project).await.expect("remove project");
    s.bus.publish(DomainEvent::ProjectRemoved { id: project });

    let root = dir.path().canonicalize().expect("canonical root");
    let _ = s.changes.send(config_path(&root));
    assert_no_config_changed(&mut s).await;
}

#[tokio::test]
async fn a_change_to_another_file_in_the_root_does_not_reload() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    s.open(dir.path());
    spawn_reactor(&s);

    // A directory that reports every file inside it also reports its siblings — only
    // `solo.yml` matters.
    let root = dir.path().canonicalize().expect("canonical root");
    let _ = s.changes.send(root.join("README.md"));
    yield_many().await;

    assert_no_config_changed(&mut s).await;
}
