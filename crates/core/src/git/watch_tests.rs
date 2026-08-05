//! Behavioural tests for [`GitStatusWatchReactor`], kept out of the implementation file. They
//! drive the real reactor over a [`FakeFileWatcher`] feeding synthetic repository-state changes
//! and a [`FakeGitRepository`] answering the reads, so what is asserted is what a surface sees:
//! how many status reads a burst caused, and what the bus announced.
//!
//! The quiet window is advanced on the mock clock, so no real debounce elapses. The reads
//! themselves run on the blocking pool, so the helpers below advance and then wait a bounded
//! moment for the announcement rather than assuming a fixed number of scheduler turns.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tokio::sync::broadcast;

use crate::events::{DomainEvent, EventBus};
use crate::git::{Git, GitError, GitStatus};
use crate::ids::ProjectId;
use crate::projects::Projects;
use crate::testing::{git_over, git_status, FakeFileWatcher, FakeGitRepository, FakeProjectRepo};

use super::GitStatusWatchReactor;

/// One advance step, comfortably past the reactor's quiet window so a single step fires it.
const STEP: Duration = Duration::from_millis(200);

/// How long to wait for the reactor to finish a read and announce before advancing again.
/// Bounded so a reactor that never announces fails the test rather than hanging.
const SETTLE: Duration = Duration::from_millis(100);

/// How many quiet windows a test will drive before giving up on an expected announcement.
const MAX_WINDOWS: usize = 40;

struct Setup {
    bus: EventBus,
    rx: broadcast::Receiver<DomainEvent>,
    clock: crate::testing::MockClock,
    watcher: Arc<FakeFileWatcher>,
    projects: Arc<Projects>,
    repository: FakeGitRepository,
    git: Arc<Git>,
    _dir: TempDir,
    project: ProjectId,
    root: PathBuf,
}

impl Setup {
    /// Feeds a synthetic change for a file inside the project's repository state, as a `git`
    /// invocation writing that file would produce.
    fn repository_wrote(&self, relative: &str) {
        self.watcher.change(self.root.join(".git").join(relative));
    }

    /// Advances the quiet window until the reactor announces a status change, and returns the
    /// project it named.
    async fn next_status_changed(&mut self) -> ProjectId {
        for window in 0..MAX_WINDOWS {
            self.clock.advance(STEP);
            let announced = tokio::time::timeout(SETTLE, self.rx.recv()).await;
            match announced {
                Ok(Ok(DomainEvent::GitStatusChanged { project })) => return project,
                Ok(Ok(_)) | Ok(Err(_)) | Err(_) => {
                    assert!(window + 1 < MAX_WINDOWS, "no status change was announced");
                }
            }
        }
        unreachable!("the loop above asserts before it runs out")
    }

    /// Drives several quiet windows and asserts the reactor announced nothing — the change was
    /// churn, or the working tree read the same as before.
    async fn assert_nothing_announced(&mut self) {
        for _ in 0..5 {
            self.clock.advance(STEP);
            tokio::time::sleep(SETTLE).await;
        }
        while let Ok(event) = self.rx.try_recv() {
            assert!(
                !matches!(event, DomainEvent::GitStatusChanged { .. }),
                "unexpected status change: {event:?}",
            );
        }
    }
}

fn setup(repository: FakeGitRepository) -> Setup {
    let bus = EventBus::new(1024);
    let rx = bus.subscribe();
    let dir = tempfile::tempdir().expect("temp dir");
    let projects = Arc::new(Projects::new(Arc::new(FakeProjectRepo::new())));
    let record = projects.add(dir.path(), None, None).expect("add project");
    Setup {
        bus,
        rx,
        clock: crate::testing::MockClock::new(),
        watcher: Arc::new(FakeFileWatcher::new()),
        projects,
        git: git_over(repository.clone()),
        repository,
        _dir: dir,
        project: record.id,
        root: record.root,
    }
}

fn clean() -> GitStatus {
    git_status("main")
}

fn on_branch(branch: &str) -> GitStatus {
    git_status(branch)
}

/// Spawns the reactor and awaits its first established watch, so the fake holds the change sink.
async fn start_reactor(s: &Setup) {
    tokio::spawn(
        GitStatusWatchReactor::new(
            Arc::new(s.clock.clone()),
            s.watcher.clone(),
            &s.bus,
            Arc::downgrade(&s.git),
            s.projects.clone(),
        )
        .run(),
    );
    s.watcher.established().await;
}

#[tokio::test]
async fn a_burst_of_repository_writes_reads_the_status_once() {
    let mut s = setup(FakeGitRepository::reporting(clean()));
    start_reactor(&s).await;

    // What one `git add` looks like from outside: several files written in quick succession.
    s.repository_wrote("index");
    s.repository_wrote("index");
    s.repository_wrote("HEAD");
    s.repository_wrote("refs/heads/main");

    assert_eq!(s.next_status_changed().await, s.project);
    s.assert_nothing_announced().await;
    assert_eq!(
        s.repository.reads(),
        1,
        "the burst coalesced into a single read",
    );
}

#[tokio::test]
async fn the_lock_files_git_writes_around_its_own_writes_are_ignored() {
    let mut s = setup(FakeGitRepository::reporting(clean()));
    start_reactor(&s).await;

    s.repository_wrote("index.lock");
    s.repository_wrote("refs/heads/main.lock");
    s.repository_wrote("packed-refs.lock");

    s.assert_nothing_announced().await;
    assert_eq!(
        s.repository.reads(),
        0,
        "a lock file is a write in progress, not a state worth reading",
    );
}

#[tokio::test]
async fn a_working_tree_that_reads_the_same_is_not_announced_again() {
    let mut s = setup(FakeGitRepository::answering(vec![Ok(clean()), Ok(clean())]));
    start_reactor(&s).await;

    s.repository_wrote("index");
    assert_eq!(s.next_status_changed().await, s.project);

    // A second write that leaves the working tree looking identical.
    s.repository_wrote("index");
    s.assert_nothing_announced().await;
    assert_eq!(
        s.repository.reads(),
        2,
        "it was read again, just not announced"
    );
}

#[tokio::test]
async fn a_read_that_loses_a_race_is_retried_once_and_then_announces() {
    let mut s = setup(FakeGitRepository::answering(vec![
        Err(GitError::Op { status: Some(128) }),
        Ok(on_branch("release")),
    ]));
    start_reactor(&s).await;

    s.repository_wrote("index");

    assert_eq!(s.next_status_changed().await, s.project);
    assert_eq!(
        s.repository.reads(),
        2,
        "the failed read was retried, and the retry is what announced",
    );
}

#[tokio::test]
async fn a_repository_that_keeps_failing_is_left_alone_after_one_retry() {
    let mut s = setup(FakeGitRepository::answering(vec![Err(GitError::Timeout)]));
    start_reactor(&s).await;

    s.repository_wrote("index");
    s.assert_nothing_announced().await;

    assert_eq!(
        s.repository.reads(),
        2,
        "one read and one retry — a repository that cannot be read is not polled",
    );
}

#[tokio::test]
async fn removing_a_project_releases_its_watches_and_stops_its_reads() {
    let mut s = setup(FakeGitRepository::reporting(clean()));
    start_reactor(&s).await;

    s.projects.remove(s.project).expect("remove project");
    s.bus.publish(DomainEvent::ProjectRemoved { id: s.project });
    // Bounded, so a reactor that holds on to its watches fails here rather than waiting for ever.
    tokio::time::timeout(SETTLE, s.watcher.released())
        .await
        .expect("the removed project's watches were released");

    assert!(
        s.watcher.live().is_empty(),
        "a removed project leaves no watch behind: {:?}",
        s.watcher.live(),
    );
    s.repository_wrote("index");
    s.assert_nothing_announced().await;
    assert_eq!(s.repository.reads(), 0);
}
