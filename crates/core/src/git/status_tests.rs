//! Behavioural tests for the git context's status cache, kept out of the implementation file.
//! They drive a real [`Git`] over the shared [`FakeGitRepository`], so what is asserted is what
//! a caller observes — the status served, how often the port was actually read, and how many
//! reads were ever in flight at once.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::git::NoopFileOpener;
use crate::git::NoopGitForge;
use crate::ids::ProjectId;
use crate::testing::{file_change, git_status, untrusting, FakeGitRepository};
use crate::vcs::{ChangeKind, FileChange, GitFileStatus, SyncState};

use super::{Git, GitLineCounts, GitStatus};
use crate::git::GitError;

/// The fake ignores it — a status read is addressed by project here, not by path.
const ROOT: &str = "/project";

fn git(repository: &FakeGitRepository) -> Git {
    Git::new(
        Arc::new(repository.clone()),
        Arc::new(NoopGitForge),
        Arc::new(NoopFileOpener),
        untrusting(),
    )
}

fn with_change(mut status: GitStatus, path: &str) -> GitStatus {
    status.changes.push(FileChange {
        path: path.to_string(),
        status: GitFileStatus {
            staged: None,
            unstaged: Some(ChangeKind::Modified),
        },
        original_path: None,
    });
    status
}

fn served(status: GitStatus) -> GitStatus {
    let repository = FakeGitRepository::reporting(status);
    git(&repository)
        .status(ProjectId::next(), Path::new(ROOT))
        .expect("read")
        .expect("a repository")
}

#[test]
fn a_second_look_is_served_without_reading_the_repository_again() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let git = git(&repository);
    let project = ProjectId::next();

    let first = git.status(project, Path::new(ROOT)).expect("read");
    let second = git.status(project, Path::new(ROOT)).expect("read");

    assert_eq!(first, second);
    assert_eq!(
        first.expect("a repository").branch.name.as_deref(),
        Some("main")
    );
    assert_eq!(
        repository.reads(),
        1,
        "the remembered status served the second look"
    );
}

#[test]
fn a_project_that_is_not_a_repository_reads_as_nothing_to_show_and_stays_that_way() {
    let repository = FakeGitRepository::answering(vec![Err(GitError::NotARepo)]);
    let git = git(&repository);
    let project = ProjectId::next();

    assert_eq!(
        git.status(project, Path::new(ROOT)).expect("no error"),
        None
    );
    assert_eq!(
        git.status(project, Path::new(ROOT)).expect("no error"),
        None
    );
    assert_eq!(
        repository.reads(),
        1,
        "a folder kept out of version control is not re-read on every look",
    );
}

#[test]
fn a_refresh_reports_a_change_only_when_the_working_tree_reads_differently() {
    let clean = git_status("main");
    let repository = FakeGitRepository::answering(vec![
        Ok(clean.clone()),
        Ok(clean.clone()),
        Ok(with_change(clean, "src/main.rs")),
    ]);
    let git = git(&repository);
    let project = ProjectId::next();

    assert!(
        git.refresh(project, Path::new(ROOT)).expect("read"),
        "the first read is new by definition",
    );
    assert!(
        !git.refresh(project, Path::new(ROOT)).expect("read"),
        "the same working tree read again is not a change",
    );
    assert!(
        git.refresh(project, Path::new(ROOT)).expect("read"),
        "a newly modified file is a change",
    );
    assert_eq!(
        git.status(project, Path::new(ROOT))
            .expect("read")
            .expect("a repository")
            .changes
            .len(),
        1,
    );
}

#[test]
fn a_failed_read_surfaces_and_leaves_the_last_known_status_standing() {
    let repository =
        FakeGitRepository::answering(vec![Ok(git_status("main")), Err(GitError::Timeout)]);
    let git = git(&repository);
    let project = ProjectId::next();

    git.refresh(project, Path::new(ROOT)).expect("first read");
    assert!(matches!(
        git.refresh(project, Path::new(ROOT)),
        Err(GitError::Timeout)
    ));

    let after = git.status(project, Path::new(ROOT)).expect("read");
    assert_eq!(
        after.expect("a repository").branch.name.as_deref(),
        Some("main"),
        "a momentary failure must not blank a rail that was showing the truth",
    );
    assert_eq!(
        repository.reads(),
        2,
        "the remembered status answered without a third read",
    );
}

#[test]
fn forgetting_a_project_makes_the_next_look_read_again() {
    let repository = FakeGitRepository::reporting(git_status("main"));
    let git = git(&repository);
    let project = ProjectId::next();

    git.status(project, Path::new(ROOT)).expect("read");
    git.forget(project);
    git.status(project, Path::new(ROOT)).expect("read");

    assert_eq!(repository.reads(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reads_against_one_repository_never_overlap() {
    // Each read dwells long enough that eight of them, released together, would overlap were
    // they not gated — so a peak of one is evidence of serialization, not of luck.
    let repository = FakeGitRepository::slow(git_status("main"), Duration::from_millis(20));
    let git = Arc::new(git(&repository));
    let project = ProjectId::next();

    let mut readers = Vec::new();
    for _ in 0..8 {
        let git = git.clone();
        readers.push(tokio::task::spawn_blocking(move || {
            git.refresh(project, Path::new(ROOT))
        }));
    }
    for reader in readers {
        reader.await.expect("reader finished").expect("read");
    }

    assert_eq!(repository.reads(), 8);
    assert_eq!(
        repository.peak_concurrent(),
        1,
        "one repository is read one call at a time",
    );
}

#[test]
fn the_status_projects_only_remote_actions_that_can_advance_the_branch() {
    let cases = [
        (None, None, SyncState::Unknown, false, false),
        (Some("topic"), None, SyncState::Unknown, false, true),
        (
            Some("topic"),
            Some("origin/topic"),
            SyncState::Unknown,
            false,
            false,
        ),
        (
            Some("topic"),
            Some("origin/topic"),
            SyncState::UpToDate,
            false,
            false,
        ),
        (
            Some("topic"),
            Some("origin/topic"),
            SyncState::Ahead { ahead: 2 },
            false,
            true,
        ),
        (
            Some("topic"),
            Some("origin/topic"),
            SyncState::Behind { behind: 2 },
            true,
            false,
        ),
        (
            Some("topic"),
            Some("origin/topic"),
            SyncState::Diverged {
                ahead: 1,
                behind: 1,
            },
            true,
            false,
        ),
    ];

    for (name, upstream, sync, pull, push) in cases {
        let mut status = git_status("topic");
        status.branch.name = name.map(str::to_string);
        status.branch.upstream = upstream.map(str::to_string);
        status.branch.sync = sync;

        let capabilities = served(status).capabilities();
        assert_eq!(capabilities.pull, pull, "pull for {sync:?}");
        assert_eq!(capabilities.push, push, "push for {sync:?}");
    }
}

#[test]
fn the_status_projects_only_working_tree_actions_that_have_an_effect() {
    let mut status = git_status("main");
    status.changes = vec![
        FileChange {
            path: "tracked.rs".into(),
            status: GitFileStatus {
                staged: None,
                unstaged: Some(ChangeKind::Modified),
            },
            original_path: None,
        },
        FileChange {
            path: "staged.rs".into(),
            status: GitFileStatus {
                staged: Some(ChangeKind::Modified),
                unstaged: None,
            },
            original_path: None,
        },
        FileChange {
            path: "new.rs".into(),
            status: GitFileStatus {
                staged: None,
                unstaged: Some(ChangeKind::Untracked),
            },
            original_path: None,
        },
    ];

    let status = served(status);
    let capabilities = status.capabilities();
    assert!(capabilities.stash, "tracked work can be set aside");
    assert_eq!(
        capabilities.discardable_paths,
        vec!["tracked.rs"],
        "only an unstaged tracked change can be restored from the index",
    );

    let disclosed = serde_json::to_value(&status).expect("status serializes");
    assert_eq!(
        disclosed["capabilities"],
        serde_json::json!({
            "pull": false,
            "push": true,
            "stash": true,
            "discardablePaths": ["tracked.rs"],
        }),
        "the wire carries the core's projection rather than asking a surface to reproduce it",
    );
}

#[test]
fn the_status_counts_paths_created_and_removed_across_both_sides_of_the_index() {
    let mut status = git_status("main");
    status.changes = vec![
        file_change(
            "added-then-edited.rs",
            Some(ChangeKind::Added),
            Some(ChangeKind::Modified),
        ),
        file_change("untracked.rs", None, Some(ChangeKind::Untracked)),
        file_change("copy.rs", Some(ChangeKind::Copied), None),
        file_change("deleted.rs", None, Some(ChangeKind::Deleted)),
        file_change("renamed.rs", Some(ChangeKind::Renamed), None),
        file_change("modified.rs", None, Some(ChangeKind::Modified)),
        file_change("type-changed.rs", Some(ChangeKind::TypeChanged), None),
        file_change("conflicted.rs", None, Some(ChangeKind::Conflicted)),
        file_change(
            "added-then-deleted.rs",
            Some(ChangeKind::Added),
            Some(ChangeKind::Deleted),
        ),
    ];

    let status = served(status);
    let counts = status.change_counts();
    assert_eq!(counts.added, 4);
    assert_eq!(counts.removed, 2);

    let disclosed = serde_json::to_value(&status).expect("status serializes");
    assert_eq!(
        disclosed["changeCounts"],
        serde_json::json!({ "added": 4, "removed": 2 }),
        "the wire carries the core's exhaustive classification",
    );
}

#[test]
fn line_totals_are_projected_without_replacing_changed_path_counts() {
    let mut status = git_status("main");
    status
        .changes
        .push(file_change("modified.rs", None, Some(ChangeKind::Modified)));
    let status = served(status.with_line_counts(GitLineCounts {
        additions: 7,
        deletions: 3,
        complete: true,
    }));

    assert_eq!(
        status.line_counts(),
        GitLineCounts {
            additions: 7,
            deletions: 3,
            complete: true,
        },
    );
    let disclosed = serde_json::to_value(&status).expect("status serializes");
    assert_eq!(
        disclosed["lineCounts"],
        serde_json::json!({ "additions": 7, "deletions": 3, "complete": true }),
    );
    assert_eq!(
        disclosed["changeCounts"],
        serde_json::json!({ "added": 0, "removed": 0 }),
        "line totals extend the projection without changing its path totals",
    );

    let mut earlier_wire = disclosed;
    earlier_wire
        .as_object_mut()
        .expect("a status is an object")
        .remove("lineCounts");
    let earlier: GitStatus = serde_json::from_value(earlier_wire).expect("older status decodes");
    assert_eq!(earlier.line_counts(), GitLineCounts::default());
}

#[test]
fn a_conflict_suppresses_actions_git_cannot_apply_to_an_unmerged_tree() {
    let mut status = git_status("main");
    status.changes.push(FileChange {
        path: "conflicted.rs".into(),
        status: GitFileStatus {
            staged: None,
            unstaged: Some(ChangeKind::Conflicted),
        },
        original_path: None,
    });

    let capabilities = served(status.clone()).capabilities();
    assert!(!capabilities.stash);
    assert!(capabilities.discardable_paths.is_empty());

    status.changes.clear();
    status.merging = true;
    status.branch.upstream = Some("origin/main".into());
    status.branch.sync = SyncState::Behind { behind: 1 };
    status.changes.push(FileChange {
        path: "resolved.rs".into(),
        status: GitFileStatus {
            staged: Some(ChangeKind::Modified),
            unstaged: None,
        },
        original_path: None,
    });

    let capabilities = served(status).capabilities();
    assert!(!capabilities.pull);
    assert!(!capabilities.stash);
}
