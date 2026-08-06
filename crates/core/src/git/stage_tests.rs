//! Behavioural tests for moving a change across the index, driving a real [`Git`] over the
//! shared [`FakeGitRepository`] so what is asserted is what a caller observes: either the
//! repository was asked to change something, or it was never reached at all.

use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::ids::ProjectId;
use crate::testing::{
    file_change, git_over, git_status, git_trusting, hunk_range, FakeGitRepository, GitChange,
};
use crate::vcs::{ChangeKind, FileChange, GitFileStatus, HunkRange};

use crate::git::GitStatus;

use super::{Git, GitWriteError};

/// The fake ignores it — a change is addressed by project here, not by path.
const ROOT: &str = "/project";

const PATH: &str = "src/main.rs";

/// The hunk every case below names, so what varies between them is never which hunk.
fn hunk() -> HunkRange {
    hunk_range(1)
}

/// A working tree holding `changes`, over a repository that accepts what it is asked to do.
fn repository(changes: Vec<FileChange>) -> FakeGitRepository {
    let mut status: GitStatus = git_status("main");
    status.changes = changes;
    FakeGitRepository::reporting(status)
}

/// A modified path and nothing else — the ordinary starting point.
fn modified() -> Vec<FileChange> {
    vec![file_change(PATH, None, Some(ChangeKind::Modified))]
}

/// Every change one path can undergo, so a rule that has to hold for all of them is stated
/// once rather than six times.
fn every_change(git: &Git, project: ProjectId, path: &str) -> Vec<GitWriteError> {
    let root = Path::new(ROOT);
    vec![
        git.stage(project, root, path).unwrap_err(),
        git.unstage(project, root, path).unwrap_err(),
        git.discard(project, root, path).unwrap_err(),
        git.stage_hunk(project, root, path, hunk()).unwrap_err(),
        git.unstage_hunk(project, root, path, hunk()).unwrap_err(),
        git.discard_hunk(project, root, path, hunk()).unwrap_err(),
    ]
}

#[test]
fn a_project_that_has_not_been_trusted_can_have_nothing_changed() {
    let repository = repository(modified());
    let git = git_over(repository.clone());
    let project = ProjectId::next();

    let refusals = every_change(&git, project, PATH);

    assert!(
        refusals
            .iter()
            .all(|refusal| matches!(refusal, GitWriteError::Untrusted)),
        "every way of changing a working tree passes the same gate: {refusals:?}",
    );
    assert_eq!(
        repository.changes(),
        Vec::new(),
        "a refused change never reaches the repository at all",
    );
}

#[test]
fn trusting_a_project_is_what_lets_it_be_changed() {
    let repository = repository(modified());
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.stage(project, Path::new(ROOT), PATH).expect("stage");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Stage {
            path: PATH.to_string(),
            original_path: None,
        }],
    );
}

#[test]
fn staging_a_renamed_path_names_where_it_came_from_as_well() {
    let renamed = FileChange {
        path: "src/renamed.rs".to_string(),
        status: GitFileStatus {
            staged: Some(ChangeKind::Renamed),
            unstaged: None,
        },
        original_path: Some("src/old.rs".to_string()),
    };
    let repository = repository(vec![renamed]);
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    git.unstage(project, Path::new(ROOT), "src/renamed.rs")
        .expect("unstage");

    assert_eq!(
        repository.changes(),
        vec![GitChange::Unstage {
            path: "src/renamed.rs".to_string(),
            original_path: Some("src/old.rs".to_string()),
        }],
        "given one name version control sees a file deleted and an unrelated one appear, and \
         records half the move",
    );
}

#[test]
fn a_path_that_climbs_out_of_the_repository_is_never_changed() {
    let repository = repository(modified());
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    let refusals = every_change(&git, project, "../../etc/passwd");

    assert!(
        refusals
            .iter()
            .all(|refusal| matches!(refusal, GitWriteError::OutsideRepository)),
        "one guard covers every way in: {refusals:?}",
    );
    assert_eq!(repository.changes(), Vec::new());
}

#[test]
fn discarding_a_path_version_control_does_not_track_is_refused_rather_than_deleting_it() {
    let untracked = vec![file_change("notes.md", None, Some(ChangeKind::Untracked))];
    let repository = repository(untracked);
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);

    let refusal = git
        .discard(project, Path::new(ROOT), "notes.md")
        .unwrap_err();

    assert!(
        matches!(refusal, GitWriteError::UntrackedPath),
        "{refusal:?}"
    );
    assert_eq!(
        repository.changes(),
        Vec::new(),
        "there is nothing in the index to restore it from, so discarding it would be a deletion",
    );
}

#[test]
fn a_hunk_is_carried_through_by_where_it_falls_rather_than_by_its_place_in_a_list() {
    let repository = repository(modified());
    let project = ProjectId::next();
    let git = git_trusting(repository.clone(), project);
    let second = HunkRange {
        old_start: 40,
        old_lines: 6,
        new_start: 41,
        new_lines: 8,
    };

    git.stage_hunk(project, Path::new(ROOT), PATH, second)
        .expect("stage hunk");

    assert_eq!(
        repository.changes(),
        vec![GitChange::StageHunk {
            path: PATH.to_string(),
            hunk: second,
        }],
    );
}

#[test]
fn a_change_and_a_read_never_run_against_one_repository_at_once() {
    let mut status: GitStatus = git_status("main");
    status.changes = modified();
    let repository = FakeGitRepository::slow(status, Duration::from_millis(30));
    let project = ProjectId::next();
    let git: Arc<Git> = git_trusting(repository.clone(), project);

    let reading = {
        let git = Arc::clone(&git);
        thread::spawn(move || {
            git.files(project, Path::new(ROOT)).ok();
        })
    };
    git.stage(project, Path::new(ROOT), PATH).expect("stage");
    reading.join().expect("read");

    assert_eq!(
        repository.peak_concurrent(),
        1,
        "a change takes the same per-project gate a read does, so the two are never inside one \
         repository together",
    );
}
