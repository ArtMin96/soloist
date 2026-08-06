//! What the adapter reports on a host with no `git` installed.
//!
//! Emptying `PATH` is what makes the tool unfindable, and that change is process-wide — so this
//! test lives in a binary of its own, where it cannot reach any other test.

use soloist_core::{GitError, GitRepository};
use soloist_git::CliGitRepository;

#[test]
fn a_host_without_git_reports_the_tool_as_missing_rather_than_a_broken_repository() {
    std::env::set_var("PATH", "");
    let dir = tempfile::tempdir().expect("temp dir");

    assert!(matches!(
        CliGitRepository::new().status(dir.path()),
        Err(GitError::GitMissing),
    ));
}
