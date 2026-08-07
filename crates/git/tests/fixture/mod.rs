//! Building real repositories for the adapter's tests.
//!
//! Nothing here is a recording: a fixture is made by the same `git` a user would run, so what
//! the tests assert is what the installed tool actually reports. Every invocation runs under an
//! identity of its own with no user or system configuration, so a fixture is the same
//! repository on every machine.
//!
//! **The fixture's own invocations are not the only ones to isolate.** The adapter under test runs
//! `git` itself, and it inherits the environment of whoever ran the tests — so the machine's real
//! configuration reaches it. That matters for exactly one thing, and it matters a great deal: a
//! configured credential helper would be run by any invocation that needs a credential, and a
//! helper is a program with access to the person's real credential store — one of them opens a
//! window and waits. So every fixture repository **resets the helper list to empty and then names a
//! stub of its own** ([`helper_consulted`]), which is version control's documented way to discard
//! the helpers a wider configuration named. Nothing a test does can reach a real credential store,
//! and the stub having run is the proof of it.

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

/// Runs `git args` in `dir` and returns what it printed, for the assertions made against
/// version control's own account of a repository rather than against the adapter's.
pub fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stderr(Stdio::null())
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// What the index holds for `path`, byte for byte — the one assertion that tells a hunk staged
/// exactly as it was produced from one staged nearly so.
pub fn staged_content(dir: &Path, path: &str) -> String {
    git_output(dir, &["show", &format!(":{path}")])
}

/// The two letters version control prints for `path` in its machine-readable status: what is
/// staged, and what the working tree holds beyond it.
pub fn porcelain_status(dir: &Path, path: &str) -> String {
    git_output(dir, &["status", "--porcelain", "--"])
        .lines()
        .find(|line| line.ends_with(path))
        .map(|line| line[..2].to_string())
        .unwrap_or_default()
}

/// Writes `contents` to `name` under `dir`, creating the folders above it.
pub fn write(dir: &Path, name: &str, contents: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, contents).expect("write file");
}

/// Where the stub credential helper is written, and where it records having been asked. Both live
/// inside `.git`, so neither is a change to the working tree that a status assertion would see.
const HELPER: &str = ".git/credential-helper-stub";
const HELPER_CONSULTED: &str = ".git/credential-helper-consulted";

/// Where the never-answering askpass program is written, inside `.git` for the same reason.
const SLOW_ASKPASS: &str = ".git/slow-askpass";

/// Points `dir` at a credential helper of its own and discards every helper the machine's own
/// configuration named.
///
/// The empty value first is what does the discarding — version control appends helpers across
/// configuration files and an empty one resets the list — so the stub is the only helper left, and
/// it records having been asked and then fails. If the reset ever stopped working, the machine's own
/// helper would run *first* and this fixture's tests would hang against it, which is the loudest
/// failure available and the point of doing it this way round.
fn isolate_credentials(dir: &Path) {
    let stub = dir.join(HELPER);
    std::fs::write(
        &stub,
        format!(
            "#!/bin/sh\nprintf 'asked\\n' >> \"$(dirname \"$0\")/{}\"\nexit 1\n",
            HELPER_CONSULTED
                .rsplit('/')
                .next()
                .expect("a file name in the path")
        ),
    )
    .expect("write the stub helper");
    std::fs::set_permissions(&stub, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("make the stub helper runnable");
    git(dir, &["config", "credential.helper", ""]);
    git(
        dir,
        &[
            "config",
            "--add",
            "credential.helper",
            &stub.to_string_lossy(),
        ],
    );
}

/// Points `dir` at a program for asking a person for a credential that takes far too long to
/// answer, which is what a window opening on a desktop nobody is looking at amounts to.
///
/// Only the exchanges that are allowed to ask a person ever reach it, so it is how the difference
/// between the two answers is observed rather than assumed.
pub fn slow_askpass(dir: &Path) {
    let stub = dir.join(SLOW_ASKPASS);
    std::fs::write(&stub, "#!/bin/sh\nsleep 600\n").expect("write the slow askpass");
    std::fs::set_permissions(&stub, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("make the slow askpass runnable");
    git(dir, &["config", "core.askPass", &stub.to_string_lossy()]);
}

/// Whether anything asked `dir` for a credential — which can only ever have been the stub, since
/// that is the one helper a fixture repository has.
pub fn helper_consulted(dir: &Path) -> bool {
    dir.join(HELPER_CONSULTED).exists()
}

/// A bare repository beside `dir`, wired up as its `origin` with the checked-out branch pushed to
/// it and tracking it. A bare repository on this disk is a real remote — every exchange with it
/// takes the same code path a remote across a network would — so the tests need no network at all.
pub fn remote_for(dir: &Path) -> TempDir {
    let remote = tempfile::tempdir().expect("temp dir");
    git(remote.path(), &["init", "--bare", "-b", BRANCH]);
    git(
        dir,
        &["remote", "add", "origin", &remote.path().to_string_lossy()],
    );
    git(dir, &["push", "--set-upstream", "origin", BRANCH]);
    remote
}

/// A second working tree cloned from `remote`, for the commits that have to arrive from somewhere
/// other than the repository under test.
pub fn clone_of(remote: &Path) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    git(
        dir.path(),
        &[
            "clone",
            &remote.to_string_lossy(),
            &dir.path().to_string_lossy(),
        ],
    );
    git(dir.path(), &["config", "user.name", "Fixture"]);
    git(dir.path(), &["config", "user.email", "fixture@example.com"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    isolate_credentials(dir.path());
    dir
}

/// Records `path`'s current contents as a commit — the shortest way for a fixture to say that
/// history moved on.
pub fn commit(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-m", message]);
}

/// What version control reports for the checked-out branch's standing, as `+<ahead> -<behind>`.
pub fn ahead_behind(dir: &Path) -> String {
    git_output(dir, &["status", "--porcelain=v2", "--branch"])
        .lines()
        .find_map(|line| line.strip_prefix("# branch.ab ").map(str::to_string))
        .unwrap_or_default()
}

/// A repository with one commit holding each of `files`. Each file gets contents of its own:
/// identical files are interchangeable to rename detection, which would pair a deletion with an
/// unrelated addition and make the fixture describe something other than what it set up.
pub fn repository_with(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    git(dir.path(), &["init", "-b", BRANCH]);
    // The adapter runs the user's own `git` without an identity of its own, so a fixture it has
    // to commit in carries one — and turns off the signing that the machine's own configuration
    // might otherwise switch on, which would make the test depend on a key nobody else has.
    git(dir.path(), &["config", "user.name", "Fixture"]);
    git(dir.path(), &["config", "user.email", "fixture@example.com"]);
    git(dir.path(), &["config", "commit.gpgsign", "false"]);
    isolate_credentials(dir.path());
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
