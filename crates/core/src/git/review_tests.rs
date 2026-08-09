//! Behavioural tests for reading an open pull request and for merging one, over the shared fakes.
//!
//! Nothing here reaches a service. What a real forge answers is the adapter's own tests' business;
//! what is asserted here is what the port was asked for, under which ceiling, and when it was not
//! reached at all.

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::git::{CheckState, MergeMethod, PullRequestError, REVIEW_LIMITS};
use crate::ids::ProjectId;
use crate::testing::{
    check_run, git_status, pull_request_review, review_thread, FakeGitForge, FakeGitRepository,
    FakeTrustRepo,
};

use super::Git;

/// The fakes ignore it — everything here is addressed by project, not by path.
const ROOT: &str = "/project";

const BRANCH: &str = "feature";

/// The pull request a merge names. Any number would do; a test reads it back to prove the one it
/// asked about is the one that was merged.
const NUMBER: u64 = 12;

/// How long a test waits for two reads that must not overlap. Long enough that a slow machine is
/// not what fails it, short enough that a gate that never releases is reported rather than waited
/// out.
const PATIENCE: Duration = Duration::from_secs(10);

/// The git context over both fakes, with `project` trusted.
fn trusting(repository: FakeGitRepository, forge: FakeGitForge, project: ProjectId) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(repository),
        Arc::new(forge),
        Arc::new(FakeTrustRepo::new().trusting_project(project)),
    ))
}

/// The same, with nothing trusted — the state every project starts in.
fn untrusting(repository: FakeGitRepository, forge: FakeGitForge) -> Arc<Git> {
    Arc::new(Git::new(
        Arc::new(repository),
        Arc::new(forge),
        Arc::new(FakeTrustRepo::new()),
    ))
}

#[test]
fn what_the_branch_has_open_comes_back_with_its_checks_and_its_conversations() {
    let forge = FakeGitForge::ready().reviewing(pull_request_review(
        BRANCH,
        vec![check_run("build", CheckState::Failed)],
        vec![review_thread("t1", "src/main.rs", 4, "this leaks")],
    ));
    let project = ProjectId::next();
    let git = trusting(
        FakeGitRepository::reporting(git_status(BRANCH)),
        forge,
        project,
    );

    let review = git
        .pull_request_review(project, Path::new(ROOT))
        .expect("read")
        .expect("the branch has one open");

    assert_eq!(review.pull_request.number, NUMBER);
    assert_eq!(review.checks[0].state, CheckState::Failed);
    assert_eq!(review.threads[0].path.as_deref(), Some("src/main.rs"));
}

#[test]
fn a_detached_head_has_nothing_open_and_the_service_is_never_asked() {
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let mut detached = git_status(BRANCH);
    detached.branch.name = None;
    let git = trusting(
        FakeGitRepository::reporting(detached),
        forge.clone(),
        project,
    );

    assert!(git
        .pull_request_review(project, Path::new(ROOT))
        .expect("read")
        .is_none());
    assert_eq!(
        forge.asks(),
        0,
        "nothing is checked out by name, so there is no branch anything could be open on",
    );
}

#[test]
fn a_review_is_read_under_the_ceiling_the_core_sets_rather_than_the_adapters_own() {
    let forge =
        FakeGitForge::ready().reviewing(pull_request_review(BRANCH, Vec::new(), Vec::new()));
    let project = ProjectId::next();
    let git = trusting(
        FakeGitRepository::reporting(git_status(BRANCH)),
        forge.clone(),
        project,
    );

    git.pull_request_review(project, Path::new(ROOT))
        .expect("read");

    assert_eq!(
        forge.review_limits(),
        vec![REVIEW_LIMITS],
        "how much of somebody else's discussion is carried is the core's bound, so the adapter \
         cannot widen it, and its two halves are different numbers so a port handed one in place \
         of the other reddens this rather than passing by coincidence",
    );
}

#[test]
fn an_untrusted_project_merges_nothing() {
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = untrusting(
        FakeGitRepository::reporting(git_status(BRANCH)),
        forge.clone(),
    );

    let refused = git
        .merge_pull_request(project, Path::new(ROOT), NUMBER, MergeMethod::Squash)
        .expect_err("an untrusted project may not merge");

    assert!(matches!(refused, PullRequestError::Untrusted));
    assert_eq!(
        forge.merged(),
        Vec::new(),
        "the gate is spent before the service is reached, so nothing was merged",
    );
}

#[test]
fn a_merge_reaches_the_service_as_the_pull_request_and_method_it_was_asked_for() {
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(
        FakeGitRepository::reporting(git_status(BRANCH)),
        forge.clone(),
        project,
    );

    git.merge_pull_request(project, Path::new(ROOT), NUMBER, MergeMethod::Rebase)
        .expect("merge");

    assert_eq!(forge.merged(), vec![(NUMBER, MergeMethod::Rebase)]);
}

#[test]
fn two_reads_of_one_project_never_reach_the_service_at_the_same_time() {
    let forge = FakeGitForge::ready()
        .reviewing(pull_request_review(BRANCH, Vec::new(), Vec::new()))
        .slow(Duration::from_millis(50));
    let project = ProjectId::next();
    let git = trusting(
        FakeGitRepository::reporting(git_status(BRANCH)),
        forge.clone(),
        project,
    );

    let started = std::time::Instant::now();
    let second = {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            git.pull_request_review(project, Path::new(ROOT))
                .expect("read")
        })
    };
    git.pull_request_review(project, Path::new(ROOT))
        .expect("read");
    second.join().expect("the second read finished");
    assert!(
        started.elapsed() < PATIENCE,
        "a gate that never releases would hold this open rather than fail it",
    );

    assert_eq!(
        forge.peak_concurrent(),
        1,
        "requests against one repository are serialized, so one read never overlaps another",
    );
}
