//! Behavioural tests for the session-scoped version-control surface — the door an agent comes
//! through. They assemble a real [`Facade`] over fakes and act as a real session, so what is
//! asserted is what a tool would get back.

use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use crate::composition::CorePorts;
use crate::facade::Facade;
use crate::git::{GitError, Prompting, SyncOp};
use crate::ids::{ProjectId, SessionId};
use crate::ports::{ProjectRepo, TokioClock, TrustRepo};
use crate::testing::{
    file_change, git_status, raw_diff, session_in_dir, FakeGitRepository, FakeProjectRepo,
    FakeSpawner, FakeTrustRepo, GitChange,
};
use crate::vcs::{ChangeKind, DiffTarget, HunkRange};

use super::ScopedGitError;
use crate::git::DiffExtent;

const PATH: &str = "src/main.rs";

const HEADER: &str =
    "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n";

/// One project's temporary root, kept alive for the test.
struct Opened {
    facade: Facade,
    trust: Arc<FakeTrustRepo>,
    repository: FakeGitRepository,
    project: ProjectId,
    root: PathBuf,
    _dirs: Vec<TempDir>,
}

impl Opened {
    /// A session whose peer sits in this project's own directory — the way an agent Soloist did not
    /// launch proves which project it belongs to.
    fn session(&self) -> SessionId {
        session_in_dir(&self.facade, self.root.clone())
    }

    /// Records that the user has trusted this project to be changed.
    fn trusted(&self) -> &Self {
        self.trust
            .set_project_trusted(self.project)
            .expect("record trust");
        self
    }
}

/// A façade over `repository` with `count` projects open, each in its own directory. Returns the
/// last one, which is deliberately **not** the only one: a surface that ignored the session's scope
/// and reached for "the one open project" would be indistinguishable with a single project open.
fn opened(repository: FakeGitRepository, count: usize) -> Opened {
    let projects = Arc::new(FakeProjectRepo::new());
    let trust = Arc::new(FakeTrustRepo::new());
    let mut dirs = Vec::new();
    let mut last = None;
    for _ in 0..count {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().canonicalize().expect("canonical root");
        let project = projects.upsert(&root, None, None).expect("add project").id;
        dirs.push(dir);
        last = Some((project, root));
    }
    let (project, root) = last.expect("at least one project");
    let facade = Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            trust.clone(),
            projects,
        )
        .git_repository(Arc::new(repository.clone()))
        .build(),
    );
    Opened {
        facade,
        trust,
        repository,
        project,
        root,
        _dirs: dirs,
    }
}

/// A façade with one project open over a repository reporting `main` and one modified path.
fn one_project() -> Opened {
    let mut status = git_status("main");
    status
        .changes
        .push(file_change(PATH, None, Some(ChangeKind::Modified)));
    opened(FakeGitRepository::reporting(status), 1)
}

#[test]
fn a_session_reads_the_repository_of_the_project_it_is_in_and_no_other() {
    let mut status = git_status("main");
    status
        .changes
        .push(file_change(PATH, None, Some(ChangeKind::Modified)));
    // Two projects are open; the session sits in the second.
    let opened = opened(FakeGitRepository::reporting(status), 2);
    let session = opened.session();

    let read = opened
        .facade
        .scoped(session)
        .git_status()
        .expect("a scoped status read");

    assert_eq!(read.branch.name.as_deref(), Some("main"));
    assert_eq!(
        opened.repository.roots(),
        vec![opened.root.clone()],
        "only the session's own project was ever read"
    );
}

#[test]
fn a_caller_with_no_project_in_scope_is_refused_before_anything_is_read() {
    // Two projects open and a session in neither directory: nothing resolves a scope, which is the
    // case a project argument would otherwise have been used to fill in.
    let opened = opened(FakeGitRepository::reporting(git_status("main")), 2);
    let elsewhere = tempfile::tempdir().expect("temp dir");
    let session = session_in_dir(
        &opened.facade,
        elsewhere.path().canonicalize().expect("canonical"),
    );

    assert!(matches!(
        opened.facade.scoped(session).git_status(),
        Err(ScopedGitError::NoProjectScope),
    ));
    assert_eq!(opened.repository.reads(), 0, "nothing was read");
    assert!(opened.repository.roots().is_empty());
}

#[test]
fn a_folder_under_no_version_control_is_named_as_such_rather_than_answered_with_nothing() {
    let opened = opened(
        FakeGitRepository::answering(vec![Err(GitError::NotARepo)]),
        1,
    );
    let session = opened.session();

    assert!(matches!(
        opened.facade.scoped(session).git_status(),
        Err(ScopedGitError::NotARepository),
    ));
    assert!(matches!(
        opened.facade.scoped(session).git_branches(),
        Err(ScopedGitError::NotARepository),
    ));
}

#[test]
fn a_project_nobody_has_trusted_reads_back_but_changes_nothing() {
    let opened = one_project();
    let session = opened.session();

    opened
        .facade
        .scoped(session)
        .git_status()
        .expect("a read is ungated");

    for refused in [
        opened.facade.scoped(session).git_stage(PATH, None),
        opened.facade.scoped(session).git_discard(PATH, None),
        opened.facade.scoped(session).git_commit("subject", false),
        opened.facade.scoped(session).git_create_branch("topic"),
        opened.facade.scoped(session).git_delete_branch("topic"),
        opened.facade.scoped(session).git_stash(),
        opened.facade.scoped(session).git_push(),
    ] {
        assert!(
            matches!(
                refused,
                Err(ScopedGitError::Change(crate::git::GitWriteError::Untrusted)),
            ),
            "an untrusted project refuses every change"
        );
    }
    assert!(
        opened.repository.changes().is_empty(),
        "nothing reached the working tree"
    );
}

#[test]
fn an_exchange_an_agent_starts_never_asks_anybody_for_a_credential() {
    let opened = one_project();
    let session = opened.session();
    opened.trusted();

    opened.facade.scoped(session).git_push().expect("push");
    opened.facade.scoped(session).git_pull().expect("pull");
    opened.facade.scoped(session).git_fetch().expect("fetch");

    let asked: Vec<(SyncOp, Prompting)> = opened
        .repository
        .changes()
        .into_iter()
        .filter_map(|change| match change {
            GitChange::Sync { op, prompting } => Some((op, prompting)),
            _ => None,
        })
        .collect();
    assert_eq!(
        asked,
        vec![
            // The branch tracks nothing, so a push is a publish — the same choice the local user's
            // push makes, from the same remembered status.
            (SyncOp::Publish, Prompting::Denied),
            (SyncOp::Pull, Prompting::Denied),
            (SyncOp::Fetch, Prompting::Denied),
        ],
        "every exchange an agent starts denies the credential prompt"
    );
}

#[test]
fn the_local_users_own_exchange_is_still_allowed_to_ask_them() {
    let opened = one_project();
    opened.trusted();

    opened
        .facade
        .git_fetch(opened.project)
        .expect("the local user's fetch");

    assert_eq!(
        opened.repository.changes(),
        vec![GitChange::Sync {
            op: SyncOp::Fetch,
            prompting: Prompting::Allowed,
        }],
        "denying the prompt is the scoped caller's answer, not everybody's"
    );
}

#[test]
fn naming_a_hunk_acts_on_that_hunk_alone_rather_than_the_whole_path() {
    let opened = one_project();
    let session = opened.session();
    opened.trusted();
    let hunk = HunkRange {
        old_start: 3,
        old_lines: 2,
        new_start: 3,
        new_lines: 4,
    };

    opened
        .facade
        .scoped(session)
        .git_stage(PATH, Some(hunk))
        .expect("stage one hunk");
    opened
        .facade
        .scoped(session)
        .git_unstage(PATH, None)
        .expect("unstage the path");

    assert_eq!(
        opened.repository.changes(),
        vec![
            GitChange::StageHunk {
                path: PATH.to_string(),
                hunk,
            },
            GitChange::Unstage {
                path: PATH.to_string(),
                original_path: None,
            },
        ],
    );
}

#[test]
fn a_change_reaches_the_working_tree_of_the_session_s_own_project() {
    let mut status = git_status("main");
    status
        .changes
        .push(file_change(PATH, None, Some(ChangeKind::Modified)));
    let opened = opened(FakeGitRepository::reporting(status), 2);
    let session = opened.session();
    opened.trusted();

    opened
        .facade
        .scoped(session)
        .git_stage(PATH, None)
        .expect("stage");

    assert!(
        opened
            .repository
            .roots()
            .iter()
            .all(|root| root == &opened.root),
        "no other project's working tree was touched: {:?}",
        opened.repository.roots(),
    );
}

#[test]
fn a_diff_of_a_path_nothing_inside_the_repository_names_is_an_absence_rather_than_a_failure() {
    let mut status = git_status("main");
    status
        .changes
        .push(file_change(PATH, None, Some(ChangeKind::Modified)));
    let repository = FakeGitRepository::reporting(status)
        .diffing(raw_diff(HEADER, &["@@ -1,1 +1,1 @@\n-old\n+new\n"]));
    let opened = opened(repository, 1);
    let session = opened.session();

    let present = opened
        .facade
        .scoped(session)
        .git_diff(PATH, DiffTarget::Unstaged, DiffExtent::Capped)
        .expect("read")
        .expect("a diff for a changed path");
    assert_eq!(present.hunks.len(), 1);

    let absent = opened
        .facade
        .scoped(session)
        .git_diff("../outside", DiffTarget::Unstaged, DiffExtent::Capped)
        .expect("read");
    assert!(
        absent.is_none(),
        "a path outside the repository has no diff"
    );
}
