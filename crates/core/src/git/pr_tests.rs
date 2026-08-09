//! Behavioural tests for proposing a branch as a pull request, over the shared fakes — so what is
//! asserted is what the two ports were asked for, or that they were never reached.
//!
//! Nothing here reaches a service. What a real forge does with a proposal is the adapter's own
//! tests' business.

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::git::{ForgeError, ForgeReadiness, GitError, SyncOp};
use crate::ids::ProjectId;
use crate::testing::{
    created_url, git_status, pull_request, pull_request_template, tracking_status, FakeGitForge,
    FakeGitRepository, FakeTrustRepo, GitChange,
};
use crate::vcs::SyncState;

use super::{Git, NewPullRequest, PullRequestError};

/// The fakes ignore it — everything here is addressed by project, not by path.
const ROOT: &str = "/project";

const BRANCH: &str = "feature";
const BASE: &str = "main";

/// How long a test waits for a proposal that should have been stopped. Long enough that a slow
/// machine is not what fails it, short enough that a signal being ignored is reported rather than
/// waited out.
const PATIENCE: Duration = Duration::from_secs(10);

/// The git context over both fakes, with `project` trusted — the starting point for a test about
/// what a proposal does rather than about whether it is allowed.
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

/// A proposal a test hands over, varied by whichever field it is about.
fn proposal() -> NewPullRequest {
    NewPullRequest {
        title: "Propose the thing".to_string(),
        body: "What it changes.".to_string(),
        base: BASE.to_string(),
        draft: false,
    }
}

#[test]
fn a_forge_that_cannot_be_reached_offers_nothing_and_is_asked_nothing() {
    for readiness in [ForgeReadiness::Missing, ForgeReadiness::LoggedOut] {
        let repository = FakeGitRepository::reporting(git_status(BRANCH));
        let forge = FakeGitForge::at(readiness);
        let project = ProjectId::next();
        let git = trusting(repository.clone(), forge.clone(), project);

        let surface = git
            .pull_request_surface(project, Path::new(ROOT), None)
            .expect("surface");

        assert_eq!(surface.readiness, readiness);
        assert_eq!(surface.head, None);
        assert_eq!(surface.base, None);
        assert_eq!(surface.templates, Vec::new());
        assert_eq!(
            (forge.asks(), repository.reads()),
            (0, 0),
            "nothing could have been answered, so nothing was asked and no repository was read",
        );
    }
}

#[test]
fn the_repositorys_own_skeletons_are_what_is_offered_when_it_carries_any() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let forge = FakeGitForge::ready().carrying(vec![
        pull_request_template("bugfix", "## The bug"),
        pull_request_template("feature", "## The feature"),
    ]);
    let project = ProjectId::next();
    let git = trusting(repository, forge, project);

    let surface = git
        .pull_request_surface(
            project,
            Path::new(ROOT),
            Some(pull_request_template("mine", "## My own shape")),
        )
        .expect("surface");

    assert_eq!(
        surface
            .templates
            .iter()
            .map(|template| template.name.as_str())
            .collect::<Vec<_>>(),
        vec!["bugfix", "feature"],
        "the repository is telling everybody who opens one what it expects, so its own convention \
         wins outright rather than being offered beside a personal shape",
    );
}

#[test]
fn the_users_own_skeleton_is_offered_only_where_the_repository_carries_none() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let project = ProjectId::next();
    let git = trusting(repository, FakeGitForge::ready(), project);

    let surface = git
        .pull_request_surface(
            project,
            Path::new(ROOT),
            Some(pull_request_template("mine", "## My own shape")),
        )
        .expect("surface");

    assert_eq!(
        surface.templates,
        vec![pull_request_template("mine", "## My own shape")],
    );
}

#[test]
fn a_repository_expecting_nothing_and_a_user_who_kept_nothing_offer_no_shape_at_all() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let project = ProjectId::next();
    let git = trusting(repository, FakeGitForge::ready(), project);

    let surface = git
        .pull_request_surface(project, Path::new(ROOT), None)
        .expect("surface");

    assert_eq!(surface.templates, Vec::new());
}

#[test]
fn the_surface_names_the_branch_it_would_propose_and_the_one_it_would_merge_into() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let forge = FakeGitForge::ready()
        .merging_into(BASE)
        .holding(pull_request(12, BRANCH));
    let project = ProjectId::next();
    let git = trusting(repository, forge, project);

    let surface = git
        .pull_request_surface(project, Path::new(ROOT), None)
        .expect("surface");

    assert_eq!(surface.head.as_deref(), Some(BRANCH));
    assert_eq!(surface.base.as_deref(), Some(BASE));
    assert_eq!(
        surface.existing.map(|existing| existing.number),
        Some(12),
        "a branch that already has one is told so, rather than being offered a second",
    );
}

#[test]
fn a_project_that_has_not_been_trusted_proposes_nothing_and_pushes_nothing() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let forge = FakeGitForge::ready();
    let git = untrusting(repository.clone(), forge.clone());
    let project = ProjectId::next();

    let refusal = git
        .create_pull_request(
            project,
            Path::new(ROOT),
            &proposal(),
            super::Prompting::Allowed,
        )
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::Untrusted),
        "{refusal:?}"
    );
    assert_eq!(forge.created(), Vec::new());
    assert_eq!(
        repository.changes(),
        Vec::new(),
        "a proposal pushes the branch, so a refused one must not have pushed it either",
    );
}

#[test]
fn a_proposal_with_nothing_but_space_for_a_title_is_refused_before_anything_runs() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(repository.clone(), forge.clone(), project);
    let blank = NewPullRequest {
        title: "   ".to_string(),
        ..proposal()
    };

    let refusal = git
        .create_pull_request(project, Path::new(ROOT), &blank, super::Prompting::Allowed)
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::EmptyTitle),
        "{refusal:?}"
    );
    assert_eq!((forge.asks(), repository.changes().len()), (0, 0));
}

#[test]
fn a_base_version_control_would_read_as_an_option_is_refused_rather_than_handed_on() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(repository, forge.clone(), project);
    let dashed = NewPullRequest {
        base: "--upload-pack=hack".to_string(),
        ..proposal()
    };

    let refusal = git
        .create_pull_request(project, Path::new(ROOT), &dashed, super::Prompting::Allowed)
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::UnusableBranchName),
        "{refusal:?}",
    );
    assert_eq!(forge.asks(), 0);
}

#[test]
fn a_branch_the_remote_has_never_seen_is_published_before_it_is_proposed() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(repository.clone(), forge.clone(), project);

    let created = git
        .create_pull_request(
            project,
            Path::new(ROOT),
            &proposal(),
            super::Prompting::Allowed,
        )
        .expect("proposed");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Sync {
            op: SyncOp::Publish,
            prompting: super::Prompting::Allowed,
        }],
        "a service can only see commits the remote holds, so the branch goes first",
    );
    assert_eq!(forge.created(), vec![proposal()]);
    assert_eq!(created, created_url());
}

#[test]
fn a_branch_ahead_of_its_upstream_hands_over_what_it_holds_before_it_is_proposed() {
    let mut status = tracking_status(BRANCH, "origin/feature");
    status.branch.sync = SyncState::Ahead { ahead: 3 };
    let repository = FakeGitRepository::reporting(status);
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(repository.clone(), forge, project);

    git.create_pull_request(
        project,
        Path::new(ROOT),
        &proposal(),
        super::Prompting::Allowed,
    )
    .expect("proposed");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Sync {
            op: SyncOp::Push,
            prompting: super::Prompting::Allowed,
        }],
    );
}

#[test]
fn a_branch_the_remote_already_holds_as_it_stands_is_proposed_without_being_pushed_again() {
    let repository = FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature"));
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(repository.clone(), forge.clone(), project);

    git.create_pull_request(
        project,
        Path::new(ROOT),
        &proposal(),
        super::Prompting::Allowed,
    )
    .expect("proposed");

    assert_eq!(
        repository.changes(),
        Vec::new(),
        "there is nothing the remote is missing, so nothing is handed to it",
    );
    assert_eq!(forge.created().len(), 1);
}

#[test]
fn a_proposal_names_the_branch_that_is_checked_out_rather_than_one_it_was_handed() {
    let repository = FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature"));
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(repository, forge.clone(), project);

    git.create_pull_request(
        project,
        Path::new(ROOT),
        &proposal(),
        super::Prompting::Allowed,
    )
    .expect("proposed");

    assert_eq!(
        forge.heads(),
        vec![BRANCH.to_string()],
        "the head is read from the repository, so no surface can propose one branch's commits \
         under another branch's name",
    );
}

#[test]
fn a_detached_head_has_no_branch_to_propose() {
    let mut status = git_status(BRANCH);
    status.branch.name = None;
    let repository = FakeGitRepository::reporting(status);
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(repository.clone(), forge.clone(), project);

    let refusal = git
        .create_pull_request(
            project,
            Path::new(ROOT),
            &proposal(),
            super::Prompting::Allowed,
        )
        .unwrap_err();

    assert!(
        matches!(refusal, PullRequestError::DetachedHead),
        "{refusal:?}",
    );
    assert_eq!((forge.asks(), repository.changes().len()), (0, 0));
}

#[test]
fn a_forge_that_refuses_says_so_in_its_own_words() {
    let repository = FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature"));
    let forge = FakeGitForge::ready().refusing(ForgeError::Refused {
        output: "a pull request for that branch already exists".to_string(),
    });
    let project = ProjectId::next();
    let git = trusting(repository, forge, project);

    let refusal = git
        .create_pull_request(
            project,
            Path::new(ROOT),
            &proposal(),
            super::Prompting::Allowed,
        )
        .unwrap_err();

    assert!(
        matches!(
            &refusal,
            PullRequestError::Forge(ForgeError::Refused { output })
                if output.contains("already exists"),
        ),
        "{refusal:?}",
    );
}

#[test]
fn a_push_that_failed_stops_the_proposal_rather_than_proposing_a_branch_nobody_can_see() {
    let repository =
        FakeGitRepository::reporting(git_status(BRANCH)).refusing(GitError::AuthFailed);
    let forge = FakeGitForge::ready();
    let project = ProjectId::next();
    let git = trusting(repository, forge.clone(), project);

    let refusal = git
        .create_pull_request(
            project,
            Path::new(ROOT),
            &proposal(),
            super::Prompting::Allowed,
        )
        .unwrap_err();

    assert!(matches!(refusal, PullRequestError::Push(_)), "{refusal:?}");
    assert_eq!(
        forge.created(),
        Vec::new(),
        "the commits never reached the remote, so there is nothing to propose",
    );
}

#[test]
fn two_readers_never_reach_the_forge_for_one_repository_at_once() {
    let repository = FakeGitRepository::reporting(git_status(BRANCH));
    let forge = FakeGitForge::ready().slow(Duration::from_millis(30));
    let project = ProjectId::next();
    let git = trusting(repository, forge.clone(), project);

    let second = {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            git.pull_request_surface(project, Path::new(ROOT), None)
                .ok();
        })
    };
    git.pull_request_surface(project, Path::new(ROOT), None)
        .expect("surface");
    second.join().expect("the second reader");

    assert_eq!(
        forge.peak_concurrent(),
        1,
        "one request per project at a time, so two surfaces opening at once cost one round of them",
    );
}

#[test]
fn stopping_a_proposal_ends_it_and_frees_the_repository_for_the_next_read() {
    // A service that accepts a connection and then says nothing: the proposal waits until it is
    // asked to stop, which is what makes both halves of this observable.
    let repository = FakeGitRepository::reporting(tracking_status(BRANCH, "origin/feature"));
    let forge = FakeGitForge::ready().stalling();
    let project = ProjectId::next();
    let git = trusting(repository, forge, project);

    let (answered, waiting) = std::sync::mpsc::channel();
    {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            let _ = answered.send(git.create_pull_request(
                project,
                Path::new(ROOT),
                &proposal(),
                super::Prompting::Allowed,
            ));
        });
    }
    let stopping = {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            git.stop_exchange(project);
        })
    };
    // The wait is bounded because the very thing under test is that the proposal ends: one that
    // ignored the signal would otherwise leave this test waiting for ever rather than failing.
    let outcome = match waiting.recv_timeout(PATIENCE) {
        Ok(outcome) => outcome,
        Err(_) => panic!("the proposal never ended, so nothing stopped it"),
    };
    stopping.join().expect("the asking thread");

    assert!(
        matches!(outcome, Err(PullRequestError::Forge(ForgeError::Stopped)),),
        "being stopped is its own outcome, not a failure: {outcome:?}",
    );
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
        "a stopped proposal releases the project's gate, so the surface reads the repository again \
         at once instead of appearing frozen behind it",
    );
}
