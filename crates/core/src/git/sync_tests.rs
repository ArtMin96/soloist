//! Behavioural tests for exchanging commits with a remote, over the shared [`FakeGitRepository`] —
//! so what is asserted is which exchange the repository was asked for, or that it was never reached.
//!
//! Nothing here reaches a network. What a real remote does with each exchange is the adapter's own
//! tests' business, against real repositories.

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::ids::ProjectId;
use crate::testing::{
    git_over, git_status, git_trusting, tracking_status, FakeGitRepository, GitChange,
};

use crate::git::GitError;

use super::{Git, GitWriteError, Progress, Prompting, SyncOp};

/// The fake ignores it — everything here is addressed by project, not by path.
const ROOT: &str = "/project";

/// How long a test waits for an exchange that should have been stopped. Long enough that a slow
/// machine is not what fails it, short enough that a signal being ignored is reported rather than
/// waited out.
const PATIENCE: Duration = Duration::from_secs(10);

/// What the local user's own door asks for: somebody is in front of the window, so a credential may
/// be asked for. The other answer is what a session-scoped caller gets.
const WATCHED: Prompting = Prompting::Allowed;

/// Every exchange with a remote, so a rule that has to hold for all of them is stated once.
fn every_exchange(git: &Git, project: ProjectId) -> Vec<GitWriteError> {
    let root = Path::new(ROOT);
    vec![
        git.fetch(project, root, WATCHED, &Progress::unwatched())
            .unwrap_err(),
        git.pull(project, root, WATCHED, &Progress::unwatched())
            .unwrap_err(),
        git.push(project, root, WATCHED, &Progress::unwatched())
            .unwrap_err(),
        git.abort_merge(project, root).unwrap_err(),
    ]
}

#[test]
fn a_project_that_has_not_been_trusted_reaches_no_remote_at_all() {
    let repository = FakeGitRepository::reporting(tracking_status("main", "origin/main"));
    let git = git_over(repository.clone());
    let project = ProjectId::next();

    let refusals = every_exchange(&git, project);

    assert!(
        refusals
            .iter()
            .all(|refusal| matches!(refusal, GitWriteError::Untrusted)),
        "a remote's address and the helper asked for a credential for it are both configuration \
         the project carries: {refusals:?}",
    );
    assert_eq!(
        repository.changes(),
        Vec::new(),
        "a refused exchange never reaches the repository, so no network is touched",
    );
}

#[test]
fn each_exchange_asks_for_the_one_it_was_named_for() {
    let repository = FakeGitRepository::reporting(tracking_status("main", "origin/main"));
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);
    let root = Path::new(ROOT);

    git.fetch(project, root, WATCHED, &Progress::unwatched())
        .expect("fetch");
    git.pull(project, root, WATCHED, &Progress::unwatched())
        .expect("pull");
    git.push(project, root, WATCHED, &Progress::unwatched())
        .expect("push");

    assert_eq!(
        repository.changes(),
        vec![
            GitChange::Sync {
                op: SyncOp::Fetch,
                prompting: WATCHED,
            },
            GitChange::Sync {
                op: SyncOp::Pull,
                prompting: WATCHED,
            },
            GitChange::Sync {
                op: SyncOp::Push,
                prompting: WATCHED,
            },
        ],
    );
}

#[test]
fn a_branch_that_tracks_nothing_is_published_rather_than_pushed() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.push(project, Path::new(ROOT), WATCHED, &Progress::unwatched())
        .expect("push");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Sync {
            op: SyncOp::Publish,
            prompting: WATCHED,
        }],
        "there is no upstream to hand the commits to, so the branch is handed over and recorded \
         as tracking it",
    );
}

#[test]
fn abandoning_a_merge_is_its_own_operation() {
    let repository = FakeGitRepository::reporting(tracking_status("main", "origin/main"));
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.abort_merge(project, Path::new(ROOT)).expect("abort");

    assert_eq!(repository.changes(), vec![GitChange::AbortMerge]);
}

#[test]
fn a_session_scoped_caller_is_told_no_credential_may_be_asked_for() {
    let repository = FakeGitRepository::reporting(tracking_status("main", "origin/main"));
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.fetch(
        project,
        Path::new(ROOT),
        Prompting::Denied,
        &Progress::unwatched(),
    )
    .expect("fetch");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Sync {
            op: SyncOp::Fetch,
            prompting: Prompting::Denied,
        }],
        "nobody is watching an agent's request, so a question nobody can answer must not be asked",
    );
}

#[test]
fn stopping_an_exchange_ends_it_and_frees_the_repository_for_the_next_read() {
    // A remote that accepts a connection and then says nothing: the exchange waits until it is
    // asked to stop, which is what makes both halves of this observable.
    let repository =
        FakeGitRepository::reporting(tracking_status("main", "origin/main")).stalling();
    let project = ProjectId::next();
    let git: Arc<Git> = git_trusting(repository, project);

    let (answered, waiting) = std::sync::mpsc::channel();
    {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            let _ =
                answered.send(git.fetch(project, Path::new(ROOT), WATCHED, &Progress::unwatched()));
        });
    }
    let stopping = {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            git.stop_exchange(project);
        })
    };
    // The wait is bounded because the very thing under test is that the exchange ends: one that
    // ignored the signal would otherwise leave this test waiting for ever rather than failing.
    let outcome = match waiting.recv_timeout(PATIENCE) {
        Ok(outcome) => outcome,
        Err(_) => panic!("the exchange never ended, so nothing stopped it"),
    };
    stopping.join().expect("the asking thread");

    assert!(
        matches!(outcome, Err(GitWriteError::Git(GitError::Stopped))),
        "being stopped is its own outcome, not a failure and not running out of time: {outcome:?}",
    );
    // The gate is what a stuck exchange holds, so proving it was released means proving the next
    // read goes straight through rather than waiting the limit out behind it.
    let read = {
        let git = Arc::clone(&git);
        let (answered, waiting) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let _ = answered.send(git.status(project, Path::new(ROOT)).is_ok());
        });
        waiting.recv_timeout(Duration::from_secs(5))
    };
    assert_eq!(
        read,
        Ok(true),
        "a stopped exchange releases the project's gate, so the rail reads the repository again \
         at once instead of appearing frozen behind it",
    );
}

#[test]
fn an_exchange_and_a_read_never_run_against_one_repository_at_once() {
    let repository = FakeGitRepository::slow(
        tracking_status("main", "origin/main"),
        Duration::from_millis(30),
    );
    let project = ProjectId::next();
    let git: Arc<Git> = git_trusting(repository.clone(), project);

    let reading = {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            git.files(project, Path::new(ROOT)).ok();
        })
    };
    git.fetch(project, Path::new(ROOT), WATCHED, &Progress::unwatched())
        .expect("fetch");
    reading.join().expect("read");

    assert_eq!(
        repository.peak_concurrent(),
        1,
        "one exchange with a remote at a time per project, and never beside a read of it",
    );
}
