//! Behavioural tests for [`GitStatusWatchReactor`], kept out of the implementation file. They
//! drive the real reactor over a [`FakeFileWatcher`] feeding synthetic repository-state and
//! working-tree changes and a [`FakeGitRepository`] answering the reads, so what is asserted is
//! what a surface sees: how many status reads a burst caused, and what the bus announced.
//!
//! The watcher reports paths, not event kinds — which kinds of filesystem event reach the core at
//! all is the adapter's decision, covered by its own integration tests over the real backend. So a
//! creation and a deletion arrive here as the same thing, a changed path, and what these tests
//! pin is that the reactor reads and announces for the working tree as well as for `.git`.
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
use crate::testing::{
    file_change, git_over, git_status, FakeFileWatcher, FakeGitRepository, FakeProjectRepo,
};
use crate::vcs::ChangeKind;

use super::GitStatusWatchReactor;

/// One advance step, comfortably past the reactor's quiet window so a single step fires it.
const STEP: Duration = Duration::from_millis(200);

/// How long to wait for the reactor to finish a read and announce before advancing again.
/// Bounded so a reactor that never announces fails the test rather than hanging.
const SETTLE: Duration = Duration::from_millis(100);

/// How many quiet windows a test will drive before giving up on an expected announcement.
const MAX_WINDOWS: usize = 40;

/// A status read that costs more than the reactor's quiet window — a huge repository, or one whose
/// read is queued behind somebody else's. What a read this slow reaches is the case where a deadline
/// computed before it has already passed by the time it comes back.
const SLOW_READ: Duration = Duration::from_millis(200);

/// A gap shorter than the reactor's quiet window: a change this soon after the last one re-arms the
/// window rather than letting it elapse.
const RESTLESS: Duration = Duration::from_millis(50);

/// A real moment for the reactor to take one change off the channel before the clock moves again —
/// what keeps a "never goes quiet" stream actually never going quiet.
const TICK: Duration = Duration::from_millis(5);

/// Enough restless steps to run well past the reactor's ceiling on postponement.
const RESTLESS_STEPS: usize = 60;

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

    /// Feeds a synthetic change for a file in the project's working tree, as editing, adding, or
    /// removing that file would produce.
    fn working_tree_changed(&self, relative: &str) {
        self.watcher.change(self.root.join(relative));
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

    /// Advances the quiet window until the repository has been read `count` times — for waiting on
    /// a read that announces nothing, where [`Self::next_status_changed`] would wait for ever.
    /// Stops as soon as the count is reached, so the clock is left where the reads left it.
    async fn reads_reach(&mut self, count: usize) {
        for window in 0..MAX_WINDOWS {
            if self.repository.reads() >= count {
                return;
            }
            self.clock.advance(STEP);
            tokio::time::sleep(SETTLE).await;
            assert!(
                window + 1 < MAX_WINDOWS,
                "the repository was never read {count} times"
            );
        }
    }

    /// Feeds a working-tree change and lets the reactor take it, then moves the clock on by less
    /// than a quiet window — so the next change lands inside the window this one opened. A stream of
    /// these is a tree that never goes quiet.
    async fn changed_without_going_quiet(&self, relative: &str) {
        self.working_tree_changed(relative);
        tokio::time::sleep(TICK).await;
        self.clock.advance(RESTLESS);
    }

    /// Waits a bounded moment **without advancing the clock** and asserts a status change was
    /// announced — for a tree whose changes never stop, where the read has to come from the ceiling
    /// on postponement rather than from a quiet window nothing ever lets elapse.
    async fn assert_announced_unprompted(&mut self) {
        tokio::time::sleep(SETTLE).await;
        let mut announced = false;
        while let Ok(event) = self.rx.try_recv() {
            announced |= matches!(event, DomainEvent::GitStatusChanged { .. });
        }
        assert!(
            announced,
            "a tree under continuous change was never re-read: coalescing became never refreshing",
        );
    }

    /// Waits a bounded moment **without advancing the clock** and asserts nothing was announced —
    /// how a test says "and this did not happen on its own", which is a different claim from
    /// [`Self::assert_nothing_announced`]'s "this did not happen".
    async fn assert_nothing_announced_unprompted(&mut self) {
        tokio::time::sleep(SETTLE).await;
        while let Ok(event) = self.rx.try_recv() {
            assert!(
                !matches!(event, DomainEvent::GitStatusChanged { .. }),
                "announced without the quiet window elapsing: {event:?}",
            );
        }
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

/// A working tree with one path changed — a status that differs from [`clean`], so reading it
/// after a change is announced rather than recognised as the same state.
fn with_change(path: &str, unstaged: ChangeKind) -> GitStatus {
    let mut status = clean();
    status.changes = vec![file_change(path, None, Some(unstaged))];
    status
}

/// Spawns the reactor and awaits the working-tree watch — the last of the three a project needs, so
/// waiting for it means every watch is in place before a test feeds a change.
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
    watch_established(s, &s.root).await;
}

/// Awaits the watch on `root`, bounded — so a reactor that never asks for it fails the test rather
/// than waiting for ever.
async fn watch_established(s: &Setup, root: &std::path::Path) {
    tokio::time::timeout(SETTLE, s.watcher.asked_for(root))
        .await
        .unwrap_or_else(|_| panic!("{} was never watched", root.display()));
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
async fn a_file_added_to_the_working_tree_announces_a_status_change() {
    let mut s = setup(FakeGitRepository::reporting(with_change(
        "notes.md",
        ChangeKind::Untracked,
    )));
    start_reactor(&s).await;

    // The working tree is watched as a tree of its own, not only the repository state inside it —
    // without that watch a file added beside `.git` touches nothing anybody is listening to.
    assert!(
        s.watcher.watched().contains(&s.root),
        "the working tree is watched, not only .git — got {:?}",
        s.watcher.watched(),
    );

    // Nothing under `.git` is touched by writing a file the repository does not track yet.
    s.working_tree_changed("notes.md");

    assert_eq!(s.next_status_changed().await, s.project);
    assert_eq!(s.repository.reads(), 1, "the new file was read");
}

#[tokio::test]
async fn a_file_deleted_from_the_working_tree_announces_a_status_change() {
    // The working tree as it reads while the file is there, then once it is gone.
    let mut s = setup(FakeGitRepository::answering(vec![
        Ok(with_change("notes.md", ChangeKind::Untracked)),
        Ok(clean()),
    ]));
    start_reactor(&s).await;

    s.working_tree_changed("notes.md");
    assert_eq!(s.next_status_changed().await, s.project);

    // Removing the file reports the path that vanished, and nothing else — so the rail follows a
    // deletion back to a clean tree rather than going on showing a file that is no longer there.
    s.working_tree_changed("notes.md");
    assert_eq!(s.next_status_changed().await, s.project);
    assert_eq!(s.repository.reads(), 2, "the deletion was read too");
}

#[tokio::test]
async fn a_burst_of_working_tree_writes_reads_the_status_once() {
    let mut s = setup(FakeGitRepository::reporting(with_change(
        "src/main.rs",
        ChangeKind::Modified,
    )));
    start_reactor(&s).await;

    // What one editor save or one agent's edit pass looks like from outside: several files
    // written across the tree in quick succession.
    s.working_tree_changed("src/main.rs");
    s.working_tree_changed("src/lib.rs");
    s.working_tree_changed("src/nested/deep.rs");
    s.working_tree_changed("README.md");

    assert_eq!(s.next_status_changed().await, s.project);
    s.assert_nothing_announced().await;
    assert_eq!(
        s.repository.reads(),
        1,
        "the burst coalesced into a single read",
    );
}

#[tokio::test]
async fn a_working_tree_that_never_stops_changing_is_still_re_read() {
    let mut s = setup(FakeGitRepository::reporting(with_change(
        "src/main.rs",
        ChangeKind::Modified,
    )));
    start_reactor(&s).await;

    // An agent writing file after file: every change lands inside the window the one before it
    // opened, so the quiet window on its own would postpone the read for as long as the agent runs —
    // which is exactly the case the rail exists for.
    for step in 0..RESTLESS_STEPS {
        s.changed_without_going_quiet(&format!("src/file{step}.rs"))
            .await;
    }

    s.assert_announced_unprompted().await;
}

#[tokio::test]
async fn a_change_shared_by_two_nested_projects_refreshes_both() {
    let mut s = setup(FakeGitRepository::reporting(with_change(
        "shared.rs",
        ChangeKind::Modified,
    )));
    // A second project rooted inside the first, as opening a repository that lives inside another
    // project's tree gives: one file, in both working trees.
    let inner_root = s.root.join("inner");
    std::fs::create_dir_all(&inner_root).expect("inner dir");
    let inner = s
        .projects
        .add(&inner_root, None, None)
        .expect("add nested project");
    start_reactor(&s).await;
    watch_established(&s, &inner_root).await;

    s.working_tree_changed("inner/shared.rs");

    let announced = [s.next_status_changed().await, s.next_status_changed().await];
    assert!(
        announced.contains(&s.project) && announced.contains(&inner.id),
        "the file is in both working trees, so both rails refresh — got {announced:?}",
    );
}

#[tokio::test]
async fn a_change_inside_an_ignored_directory_announces_nothing() {
    let mut s = setup(FakeGitRepository::reporting(clean()));
    start_reactor(&s).await;

    s.working_tree_changed("node_modules/left-pad/index.js");
    s.working_tree_changed("target/debug/soloist");
    s.working_tree_changed("dist/bundle.js");

    s.assert_nothing_announced().await;
    assert_eq!(
        s.repository.reads(),
        0,
        "a build or dependency tree's churn is not a working-tree change",
    );
}

#[tokio::test]
async fn one_operation_touching_both_the_working_tree_and_the_repository_reads_the_status_once() {
    let mut s = setup(FakeGitRepository::reporting(with_change(
        "src/main.rs",
        ChangeKind::Modified,
    )));
    start_reactor(&s).await;

    // One `git add`: the file it staged, and the index it wrote.
    s.working_tree_changed("src/main.rs");
    s.repository_wrote("index");

    assert_eq!(s.next_status_changed().await, s.project);
    s.assert_nothing_announced().await;
    assert_eq!(
        s.repository.reads(),
        1,
        "the two watches share one quiet window, so one operation is one read",
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
async fn a_retry_waits_out_a_quiet_window_even_when_the_read_it_follows_outlasted_one() {
    let mut s = setup(FakeGitRepository::answering(vec![
        Err(GitError::Op { status: Some(128) }),
        Ok(on_branch("release")),
    ]));
    // The read itself takes longer than the quiet window, which is exactly when losing the
    // `index.lock` race is most likely — and so exactly when the retry must not go straight back in.
    s.repository.each_read_takes(s.clock.clone(), SLOW_READ);
    start_reactor(&s).await;

    s.repository_wrote("index");
    s.reads_reach(1).await;

    // The retry is armed for a window measured from now, not from before the read — so it has not
    // come due yet, and nothing has been read again.
    s.assert_nothing_announced_unprompted().await;
    assert_eq!(
        s.repository.reads(),
        1,
        "the retry re-read without waiting out any quiet window at all",
    );

    // It does come, once a quiet window actually elapses.
    assert_eq!(s.next_status_changed().await, s.project);
    assert_eq!(s.repository.reads(), 2, "the retry is what announced");
}

#[tokio::test]
async fn a_working_tree_that_cannot_be_watched_still_reports_what_version_control_writes() {
    let mut s = setup(FakeGitRepository::answering(vec![
        Ok(clean()),
        Ok(on_branch("release")),
    ]));
    // The tree watch spends one OS watch per directory beneath it, so it is the one an exhausted
    // watch budget turns down first — while the repository state beside it costs almost nothing.
    // Which is why the state is watched in its own right rather than left to the tree watch that
    // spans it: staging and committing have to survive losing the expensive half.
    s.watcher.refuse(s.root.clone());
    start_reactor(&s).await;

    // A file directly inside the repository state, as staging writes.
    s.repository_wrote("index");
    assert_eq!(s.next_status_changed().await, s.project, "staging reports");

    // And one nested inside it, as moving a branch writes — a different watch again, because a ref
    // sits levels down.
    s.repository_wrote("refs/heads/main");
    assert_eq!(
        s.next_status_changed().await,
        s.project,
        "a branch moving reports",
    );

    // The half that was lost is honestly lost: the tree reports nothing, which is what the refusal
    // is traced for.
    s.working_tree_changed("notes.md");
    s.assert_nothing_announced().await;
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
