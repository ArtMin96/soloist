//! Behavioural tests for [`ConfigWatchReactor`], kept out of the implementation file. They
//! drive a real [`ConfigEngine`] and [`Supervisor`] over fakes plus a [`FakeFileWatcher`]
//! feeding synthetic change events against a real `solo.yml` on disk — the events are
//! synthetic, the file contents are real, so the hash-diff path is the production one.
//! Waits are event-driven and the debounce window is advanced on the mock clock, so there
//! is no OS watcher and no reliance on real time.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::broadcast;

use crate::composition::CorePorts;
use crate::config::{config_path, ConfigEngine};
use crate::events::{DomainEvent, EventBus};
use crate::ids::ProjectId;
use crate::process::ProcStatus;
use crate::supervisor::Supervisor;
use crate::testing::{
    next_matching, FakeFileWatcher, FakeProjectRepo, FakeSpawner, FakeTrustRepo, MockClock,
};

use super::{ConfigWatchReactor, ProjectService, Projects};

/// One advance step, comfortably past the reactor's quiet window so a single step fires it.
const STEP: Duration = Duration::from_millis(400);

struct Setup {
    projects: Arc<Projects>,
    config: Arc<ConfigEngine>,
    sup: Arc<Supervisor>,
    bus: EventBus,
    rx: broadcast::Receiver<DomainEvent>,
    clock: MockClock,
    watcher: Arc<FakeFileWatcher>,
}

impl Setup {
    fn service(&self) -> ProjectService<'_> {
        ProjectService::new(&self.projects, &self.config, &self.sup, &self.bus)
    }

    /// Opens the project rooted at `dir` and returns its id.
    fn open(&self, dir: &Path) -> ProjectId {
        self.service().open(dir).expect("open project").id
    }

    /// Feeds a burst of synthetic change events for the project's `solo.yml`, as an editor
    /// save would produce. Canonicalized, because the OS watcher reports real paths under
    /// the canonical root the registry stores.
    fn config_changed_burst(&self, dir: &Path) {
        let root = dir.canonicalize().expect("canonical root");
        for _ in 0..5 {
            self.watcher.change(config_path(&root));
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
    Setup {
        projects: Arc::new(Projects::new(repo)),
        config: Arc::new(ConfigEngine::new(trust, bus.clone())),
        sup,
        bus,
        rx,
        clock,
        watcher: Arc::new(FakeFileWatcher::new()),
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

/// Spawns the reactor without waiting — for asserting nothing is watched.
fn spawn_reactor(s: &Setup) {
    tokio::spawn(
        ConfigWatchReactor::new(
            Arc::new(s.clock.clone()),
            s.watcher.clone(),
            &s.bus,
            Arc::downgrade(&s.sup),
            s.projects.clone(),
            s.config.clone(),
        )
        .run(),
    );
}

/// Spawns the reactor and awaits its first established watch (so the fake holds the change
/// sink). Use [`spawn_reactor`] directly when no watch is expected.
async fn start_reactor(s: &Setup) {
    spawn_reactor(s);
    s.watcher.established().await;
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
    start_reactor(&s).await;

    // An external editor changes the command and saves — several FS events, one edit.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: echo two\n");
    s.config_changed_burst(dir.path());
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
    start_reactor(&s).await;

    // The added command asks for auto-start — sync updates config only, and the variant is
    // untrusted, so it must appear resting, never running.
    write_yml(
        dir.path(),
        "processes:\n  Echo:\n    command: echo one\n  Extra:\n    command: sleep 5\n    auto_start: true\n",
    );
    s.config_changed_burst(dir.path());
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
    start_reactor(&s).await;

    // A tool re-saves the file without changing it: events fire, the hash matches.
    write_yml(dir.path(), yml);
    s.config_changed_burst(dir.path());
    yield_many().await;

    assert_no_config_changed(&mut s).await;
}

#[tokio::test]
async fn the_apps_own_write_does_not_reload_again() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    let project = s.open(dir.path());
    start_reactor(&s).await;

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
    s.config_changed_burst(dir.path());
    yield_many().await;

    assert_no_config_changed(&mut s).await;
}

#[tokio::test]
async fn an_invalid_edit_is_ignored_until_the_file_is_valid_again() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    s.open(dir.path());
    start_reactor(&s).await;

    // A mid-edit save is broken YAML: the reload fails quietly, the config keeps its last
    // good state, and the reactor stays alive.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: [broken\n");
    s.config_changed_burst(dir.path());
    yield_many().await;
    assert_no_config_changed(&mut s).await;

    // The next valid save syncs normally — the failure was transient, not sticky.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: echo two\n");
    s.config_changed_burst(dir.path());
    yield_many().await;

    let DomainEvent::ConfigChanged { diff, .. } = next_config_changed(&mut s).await else {
        unreachable!()
    };
    assert_eq!(diff.updated, vec!["Echo".to_string()]);
}

#[tokio::test]
async fn a_project_opened_after_startup_is_watched() {
    let mut s = setup();
    // Nothing is open, so nothing is watched.
    spawn_reactor(&s);
    yield_many().await;
    assert!(
        s.watcher.watched().is_empty(),
        "there is nothing to watch at startup",
    );

    // Opening a project announces it; the reactor re-syncs and watches its root.
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    s.open(dir.path());
    s.watcher.established().await;

    // An external edit to the newly-watched config now reloads, proving the watch is live.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: echo two\n");
    s.config_changed_burst(dir.path());
    yield_many().await;
    let DomainEvent::ConfigChanged { diff, .. } = next_config_changed(&mut s).await else {
        unreachable!()
    };
    assert_eq!(diff.updated, vec!["Echo".to_string()]);
}

#[tokio::test]
async fn reopening_a_project_re_establishes_its_watch() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    let project = s.open(dir.path());
    start_reactor(&s).await;
    assert_eq!(
        s.watcher.watched().len(),
        1,
        "the open established exactly one watch",
    );

    // A re-open of the same project — the folder picker on an open project, or a forwarded
    // second launch — republishes ProjectOpened. Opening a path can mean its directory was
    // replaced (a new inode silently kills the OS watch), so the reactor must drop the old
    // watch and establish a fresh one rather than trust the handle it holds.
    s.bus.publish(DomainEvent::ProjectOpened { id: project });
    s.watcher.established().await;
    assert_eq!(
        s.watcher.watched().len(),
        2,
        "the re-open re-established the watch instead of keeping the stale one",
    );

    // The re-established watch is live and drives a reload: an edit after the re-open is seen.
    write_yml(dir.path(), "processes:\n  Echo:\n    command: echo two\n");
    s.config_changed_burst(dir.path());
    yield_many().await;
    let DomainEvent::ConfigChanged { diff, .. } = next_config_changed(&mut s).await else {
        unreachable!()
    };
    assert_eq!(diff.updated, vec!["Echo".to_string()]);
}

#[tokio::test]
async fn a_removed_projects_config_watch_is_released() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    let project = s.open(dir.path());
    start_reactor(&s).await;
    assert_eq!(s.watcher.live().len(), 1);

    // Removing the project announces it; the reactor re-syncs and drops the root's watch,
    // releasing its OS resources.
    s.service().remove(project).await.expect("remove project");
    s.watcher.released().await;
    assert!(
        s.watcher.live().is_empty(),
        "the removed project's watch is dropped",
    );

    // A late event for the removed project's config is not a reload — nothing matches it.
    s.config_changed_burst(dir.path());
    yield_many().await;
    assert_no_config_changed(&mut s).await;
}

#[tokio::test]
async fn a_change_to_another_file_in_the_root_does_not_reload() {
    let mut s = setup();
    let dir = project_dir("processes:\n  Echo:\n    command: echo one\n");
    s.open(dir.path());
    start_reactor(&s).await;

    // The non-recursive root watch also reports sibling files — only `solo.yml` matters.
    let root = dir.path().canonicalize().expect("canonical root");
    s.watcher.change(root.join("README.md"));
    yield_many().await;

    assert_no_config_changed(&mut s).await;
}
