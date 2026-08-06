//! Building real repositories for the adapter's tests.
//!
//! Nothing here is a recording: a fixture is made by the same `git` a user would run, so what
//! the tests assert is what the installed tool actually reports. Every invocation runs under an
//! identity of its own with no user or system configuration, so a fixture is the same
//! repository on every machine.

// Each integration test binary compiles this module separately, so a helper only some of them
// build a repository with reads as unused in the others.
#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// The branch every fixture starts on, named rather than inherited so the assertions do not
/// depend on whichever default the host's git was built with.
pub const BRANCH: &str = "main";

/// Runs `git args` in `dir`, reporting whether it succeeded — for the invocations a fixture
/// *wants* to fail.
pub fn try_git(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run git")
        .success()
}

/// Runs `git args` in `dir`, failing the test if the fixture step did not take.
pub fn git(dir: &Path, args: &[&str]) {
    assert!(
        try_git(dir, args),
        "git {args:?} failed in {}",
        dir.display()
    );
}

/// Writes `contents` to `name` under `dir`, creating the folders above it.
pub fn write(dir: &Path, name: &str, contents: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, contents).expect("write file");
}

/// A repository with one commit holding each of `files`. Each file gets contents of its own:
/// identical files are interchangeable to rename detection, which would pair a deletion with an
/// unrelated addition and make the fixture describe something other than what it set up.
pub fn repository_with(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-b", BRANCH]);
    for file in files {
        write(
            dir.path(),
            file,
            &format!("the original contents of {file}\n"),
        );
    }
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-m", "start"]);
    dir
}
