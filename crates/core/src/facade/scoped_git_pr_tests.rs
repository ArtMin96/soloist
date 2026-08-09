//! Behavioural tests for the session-scoped pull-request surface: what an agent can read about a
//! proposal, what it may propose or merge, and the one thing it may never do — have somebody asked
//! for a credential.

use std::sync::{Arc, Mutex};

use tempfile::TempDir;

use crate::composition::CorePorts;
use crate::facade::Facade;
use crate::git::{
    ForgeReadiness, MergeMethod, NewPullRequest, Progress, Prompting, PullRequestError, SyncOp,
};
use crate::ids::{ProjectId, SessionId};
use crate::ports::{ProjectRepo, TokioClock, TrustRepo};
use crate::testing::{
    created_url, git_status, session_in_dir, FakeGitForge, FakeGitRepository, FakeProjectRepo,
    FakeSpawner, FakeTrustRepo, GitChange,
};

use super::ScopedGitError;

/// A façade over `repository` and `forge` with one project open, plus a session sitting in it.
struct Opened {
    facade: Facade,
    trust: Arc<FakeTrustRepo>,
    repository: FakeGitRepository,
    forge: FakeGitForge,
    project: ProjectId,
    session: SessionId,
    _dir: TempDir,
}

impl Opened {
    fn trusted(&self) -> &Self {
        self.trust
            .set_project_trusted(self.project)
            .expect("record trust");
        self
    }
}

fn opened(repository: FakeGitRepository, forge: FakeGitForge) -> Opened {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().canonicalize().expect("canonical root");
    let projects = Arc::new(FakeProjectRepo::new());
    let project = projects.upsert(&root, None, None).expect("add project").id;
    let trust = Arc::new(FakeTrustRepo::new());
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            trust.clone(),
            projects,
        )
        .git_repository(Arc::new(repository.clone()))
        .git_forge(Arc::new(forge.clone()))
        .build(),
    );
    let session = session_in_dir(&facade, root);
    Opened {
        facade,
        trust,
        repository,
        forge,
        project,
        session,
        _dir: dir,
    }
}

fn proposal() -> NewPullRequest {
    NewPullRequest {
        title: "Add the thing".into(),
        body: "It does the thing.".into(),
        base: "main".into(),
        draft: false,
    }
}

#[test]
fn the_surface_says_what_could_be_proposed_and_onto_what() {
    let opened = opened(
        FakeGitRepository::reporting(git_status("topic")),
        FakeGitForge::ready()
            .merging_into("main")
            .allowing(vec![MergeMethod::Squash]),
    );

    let surface = opened
        .facade
        .scoped(opened.session)
        .git_pull_request()
        .expect("a scoped surface read");

    assert_eq!(surface.readiness, ForgeReadiness::Ready);
    assert_eq!(surface.head.as_deref(), Some("topic"));
    assert_eq!(surface.base.as_deref(), Some("main"));
    assert_eq!(surface.merge_methods, vec![MergeMethod::Squash]);
}

#[test]
fn a_forge_that_cannot_be_reached_is_reported_rather_than_asked() {
    let opened = opened(
        FakeGitRepository::reporting(git_status("topic")),
        FakeGitForge::at(ForgeReadiness::LoggedOut),
    );

    let surface = opened
        .facade
        .scoped(opened.session)
        .git_pull_request()
        .expect("readiness is an answer, not a failure");

    assert_eq!(surface.readiness, ForgeReadiness::LoggedOut);
    assert_eq!(opened.forge.asks(), 0, "nothing was asked of it");
}

#[test]
fn a_proposal_an_agent_makes_never_asks_anybody_for_a_credential() {
    let opened = opened(
        FakeGitRepository::reporting(git_status("topic")),
        FakeGitForge::ready(),
    );
    opened.trusted();

    let url = opened
        .facade
        .scoped(opened.session)
        .git_create_pull_request(&proposal())
        .expect("propose");

    assert_eq!(url, created_url());
    // The branch tracks nothing, so it is published first — and that publish is the exchange whose
    // prompting decision this is about.
    assert_eq!(
        opened.repository.changes(),
        vec![GitChange::Sync {
            op: SyncOp::Publish,
            prompting: Prompting::Denied,
        }],
    );
}

#[test]
fn a_project_nobody_has_trusted_proposes_and_merges_nothing() {
    let opened = opened(
        FakeGitRepository::reporting(git_status("topic")),
        FakeGitForge::ready(),
    );

    assert!(matches!(
        opened
            .facade
            .scoped(opened.session)
            .git_create_pull_request(&proposal()),
        Err(ScopedGitError::PullRequest(PullRequestError::Untrusted)),
    ));
    assert!(matches!(
        opened.facade.scoped(opened.session).git_merge_pull_request(
            7,
            MergeMethod::Squash,
            &Progress::unwatched()
        ),
        Err(ScopedGitError::PullRequest(PullRequestError::Untrusted)),
    ));
    assert!(opened.forge.created().is_empty());
    assert!(opened.forge.merged().is_empty());
}

#[test]
fn merging_reaches_the_service_with_the_method_that_was_asked_for() {
    let opened = opened(
        FakeGitRepository::reporting(git_status("topic")),
        FakeGitForge::ready(),
    );
    opened.trusted();

    opened
        .facade
        .scoped(opened.session)
        .git_merge_pull_request(7, MergeMethod::Rebase, &Progress::unwatched())
        .expect("merge");

    assert_eq!(opened.forge.merged(), vec![(7, MergeMethod::Rebase)]);
}

#[test]
fn a_branch_with_nothing_open_reads_back_as_having_nothing_open() {
    let opened = opened(
        FakeGitRepository::reporting(git_status("topic")),
        FakeGitForge::ready(),
    );

    let review = opened
        .facade
        .scoped(opened.session)
        .git_pull_request_review()
        .expect("a scoped review read");

    assert!(review.is_none());
}

#[test]
fn a_merge_an_agent_asked_to_be_told_about_hears_what_the_service_is_doing() {
    let said = "Merging pull request #7";
    let opened = opened(
        FakeGitRepository::reporting(git_status("topic")),
        FakeGitForge::ready().saying(&[said]),
    );
    opened.trusted();
    let heard = Arc::new(Mutex::new(Vec::new()));
    let collecting = Arc::clone(&heard);
    let progress = Progress::watched_by(Arc::new(move |remark: &str| {
        collecting
            .lock()
            .expect("nothing panics holding this")
            .push(remark.to_string())
    }));

    opened
        .facade
        .scoped(opened.session)
        .git_merge_pull_request(7, MergeMethod::Rebase, &progress)
        .expect("merge");

    assert_eq!(
        *heard.lock().expect("nothing panics holding this"),
        vec![said.to_string()],
        "a merge an agent is waiting on told it nothing while it ran",
    );
}
