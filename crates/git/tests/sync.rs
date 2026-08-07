//! What exchanging commits with a real remote does, asserted against version control's own account
//! of both sides.
//!
//! No network is involved and none is needed: a bare repository on this disk is a real remote, and
//! the one test about credentials talks to a listener of its own making that answers every request
//! by demanding them. Nothing here reaches anything outside the machine it runs on.

mod fixture;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use fixture::{
    ahead_behind, clone_of, commit, git, git_output, helper_consulted, remote_for, repository_with,
    slow_askpass, write,
};

use soloist_core::{Exchange, GitError, GitRepository, Prompting, Stop, SyncOp};
use soloist_git::CliGitRepository;

/// How long a test allows an exchange that should fail at once to take before calling it a hang. Far
/// under the adapter's own network limit, because the point is that a credential nobody can supply
/// fails at once rather than being waited out.
const PROMPTLY: Duration = Duration::from_secs(20);

/// One exchange as the local user's own door makes it: a person is at the window, so version control
/// may ask them, and nothing has been stopped.
fn watched(op: SyncOp, stop: &Stop) -> Exchange<'_> {
    Exchange {
        op,
        prompting: Prompting::Allowed,
        stop,
    }
}

/// One exchange as a session-scoped caller's door makes it: nobody is watching.
fn unattended(op: SyncOp, stop: &Stop) -> Exchange<'_> {
    Exchange {
        op,
        prompting: Prompting::Denied,
        stop,
    }
}

#[test]
fn pushing_hands_the_branchs_commits_to_the_remote_and_squares_the_standing() {
    let dir = repository_with(&["a.txt"]);
    let remote = remote_for(dir.path());
    write(dir.path(), "a.txt", "changed here\n");
    commit(dir.path(), "a commit the remote has not seen");
    assert_eq!(ahead_behind(dir.path()), "+1 -0");

    CliGitRepository::new()
        .sync(dir.path(), watched(SyncOp::Push, &Stop::default()))
        .expect("push");

    assert_eq!(
        ahead_behind(dir.path()),
        "+0 -0",
        "the standing is true again with no separate refresh",
    );
    assert_eq!(
        git_output(remote.path(), &["log", "-1", "--format=%s"]).trim(),
        "a commit the remote has not seen",
        "and the remote holds the commit",
    );
}

#[test]
fn publishing_a_branch_that_tracks_nothing_gives_it_an_upstream_to_track() {
    let dir = repository_with(&["a.txt"]);
    let remote = remote_for(dir.path());
    git(dir.path(), &["switch", "--create", "feature"]);
    write(dir.path(), "b.txt", "new work\n");
    commit(dir.path(), "work on a brand-new branch");
    assert!(
        !git_output(dir.path(), &["status", "--porcelain=v2", "--branch"])
            .contains("branch.upstream"),
        "the fixture starts with a branch that tracks nothing",
    );

    CliGitRepository::new()
        .sync(dir.path(), watched(SyncOp::Publish, &Stop::default()))
        .expect("publish");

    assert!(
        git_output(dir.path(), &["status", "--porcelain=v2", "--branch"])
            .contains("# branch.upstream origin/feature")
    );
    assert!(git_output(remote.path(), &["branch", "--list"]).contains("feature"));
}

#[test]
fn fetching_makes_the_standing_true_without_touching_the_working_tree() {
    let dir = repository_with(&["a.txt"]);
    let remote = remote_for(dir.path());
    let other = clone_of(remote.path());
    write(other.path(), "a.txt", "changed elsewhere\n");
    commit(other.path(), "somebody else's commit");
    git(other.path(), &["push"]);
    let before = std::fs::read_to_string(dir.path().join("a.txt")).expect("read");

    CliGitRepository::new()
        .sync(dir.path(), watched(SyncOp::Fetch, &Stop::default()))
        .expect("fetch");

    assert_eq!(ahead_behind(dir.path()), "+0 -1");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("read"),
        before,
        "a fetch brings commits in, not into the working tree",
    );
}

#[test]
fn pulling_brings_the_remotes_commits_into_the_working_tree() {
    let dir = repository_with(&["a.txt"]);
    let remote = remote_for(dir.path());
    let other = clone_of(remote.path());
    write(other.path(), "b.txt", "added elsewhere\n");
    commit(other.path(), "somebody else's file");
    git(other.path(), &["push"]);

    CliGitRepository::new()
        .sync(dir.path(), watched(SyncOp::Pull, &Stop::default()))
        .expect("pull");

    assert!(dir.path().join("b.txt").exists());
    assert_eq!(ahead_behind(dir.path()), "+0 -0");
}

#[test]
fn a_divergence_the_user_has_not_said_how_to_reconcile_is_refused_rather_than_guessed_at() {
    let dir = repository_with(&["a.txt"]);
    let remote = remote_for(dir.path());
    let other = clone_of(remote.path());
    write(other.path(), "a.txt", "their line\n");
    commit(other.path(), "their commit");
    git(other.path(), &["push"]);
    write(dir.path(), "a.txt", "our line\n");
    commit(dir.path(), "our commit");
    let head = git_output(dir.path(), &["rev-parse", "HEAD"]);

    let refusal = CliGitRepository::new()
        .sync(dir.path(), watched(SyncOp::Pull, &Stop::default()))
        .unwrap_err();

    let GitError::Refused { output } = refusal else {
        panic!("expected version control's own account of the divergence: {refusal:?}");
    };
    assert!(output.contains("divergent branches"), "{output}");
    assert_eq!(
        git_output(dir.path(), &["rev-parse", "HEAD"]),
        head,
        "nothing was merged, rebased, or reset on the user's behalf",
    );
}

#[test]
fn a_pull_that_conflicts_leaves_the_conflict_to_be_resolved_and_says_a_merge_is_under_way() {
    let dir = repository_with(&["a.txt"]);
    let remote = remote_for(dir.path());
    let other = clone_of(remote.path());
    write(other.path(), "a.txt", "their line\n");
    commit(other.path(), "their commit");
    git(other.path(), &["push"]);
    write(dir.path(), "a.txt", "our line\n");
    commit(dir.path(), "our commit");
    // The user's own configuration decides how a divergence is reconciled; this fixture is one who
    // has said "merge", which is the case that can conflict.
    git(dir.path(), &["config", "pull.rebase", "false"]);
    let repository = CliGitRepository::new();

    let refusal = repository
        .sync(dir.path(), watched(SyncOp::Pull, &Stop::default()))
        .unwrap_err();

    assert!(
        matches!(refusal, GitError::Refused { .. }),
        "a conflict is an outcome to report, not a success: {refusal:?}",
    );
    let status = repository.status(dir.path()).expect("status");
    assert!(status.merging, "a merge is under way and can be abandoned");
    let conflicted: Vec<&str> = status
        .changes
        .iter()
        .filter(|change| change.status.unstaged == Some(soloist_core::ChangeKind::Conflicted))
        .map(|change| change.path.as_str())
        .collect();
    assert_eq!(conflicted, vec!["a.txt"]);

    repository.abort_merge(dir.path()).expect("abort");

    let after = repository.status(dir.path()).expect("status");
    assert!(!after.merging);
    assert_eq!(
        after.changes,
        Vec::new(),
        "abandoning it restores what was checked out before the merge began",
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a.txt")).expect("read"),
        "our line\n",
    );
}

#[test]
fn abandoning_a_merge_that_is_not_under_way_says_so_rather_than_reporting_success() {
    let dir = repository_with(&["a.txt"]);

    let refusal = CliGitRepository::new().abort_merge(dir.path()).unwrap_err();

    assert!(
        matches!(refusal, GitError::Refused { .. }),
        "the working tree was not what the caller thought it was: {refusal:?}",
    );
}

#[test]
fn a_remote_that_demands_a_credential_nobody_can_supply_fails_promptly_rather_than_waiting() {
    let dir = repository_with(&["a.txt"]);
    let port = serve_demanding_credentials();
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/repo.git"),
        ],
    );
    let started = Instant::now();

    let refusal = CliGitRepository::new()
        .sync(dir.path(), unattended(SyncOp::Publish, &Stop::default()))
        .unwrap_err();

    assert_eq!(
        refusal,
        GitError::AuthFailed,
        "an unreachable remote and a refused credential share an exit status, so the difference \
         has to be read out of what version control said",
    );
    assert!(
        started.elapsed() < PROMPTLY,
        "there is no terminal to type a credential into and no window to show one in, so waiting \
         for one would be waiting for ever: took {:?}",
        started.elapsed(),
    );
}

#[test]
fn a_credential_only_a_person_could_give_is_asked_for_where_one_is_watching_and_can_be_stopped() {
    let dir = repository_with(&["a.txt"]);
    let port = serve_demanding_credentials();
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/repo.git"),
        ],
    );
    // A program that would ask a person and never come back — a window on a desktop, in effect.
    slow_askpass(dir.path());
    let stop = Stop::default();
    let asking = {
        let stop = stop.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            stop.stop();
        })
    };
    let started = Instant::now();

    let outcome = CliGitRepository::new().sync(dir.path(), watched(SyncOp::Publish, &stop));
    asking.join().expect("the asking thread");

    assert_eq!(
        outcome.err(),
        Some(GitError::Stopped),
        "somebody is at the window, so version control was allowed to ask them — and the wait for \
         their answer is what stopping exists to end",
    );
    assert!(
        started.elapsed() < PROMPTLY,
        "stopping has to end the wait rather than let it run to the limit: took {:?}",
        started.elapsed(),
    );
    assert!(
        helper_consulted(dir.path()),
        "the only helper a fixture has is its own stub, and it was the one asked — nothing \
         reached a real credential store",
    );
}

#[test]
fn an_exchange_nobody_can_answer_for_is_not_asked_about_at_all() {
    let dir = repository_with(&["a.txt"]);
    let port = serve_demanding_credentials();
    git(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/repo.git"),
        ],
    );
    slow_askpass(dir.path());
    let started = Instant::now();

    let refusal = CliGitRepository::new()
        .sync(dir.path(), unattended(SyncOp::Publish, &Stop::default()))
        .unwrap_err();

    assert_eq!(
        refusal,
        GitError::AuthFailed,
        "nobody is watching, so the same program that would have been asked is not asked at all",
    );
    assert!(
        started.elapsed() < PROMPTLY,
        "which is the whole point: it fails instead of waiting: took {:?}",
        started.elapsed(),
    );
}

/// Serves HTTP on a port of the operating system's choosing, answering every request by demanding
/// credentials — which is what a remote does to an anonymous push. Returns the port.
///
/// The thread outlives the test deliberately: it holds nothing the test owns, and the test binary
/// exiting is what closes the listener.
fn serve_demanding_credentials() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listen");
    let port = listener.local_addr().expect("address").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(
                b"HTTP/1.1 401 Unauthorized\r\n\
                  WWW-Authenticate: Basic realm=\"git\"\r\n\
                  Content-Length: 0\r\n\r\n",
            );
        }
    });
    port
}
