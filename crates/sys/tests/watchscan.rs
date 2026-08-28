//! Integration check against the real `ignore`-crate walk: the set of paths this scanner reports
//! for a real directory tree matches what `git status` itself would read, over a real
//! `.gitignore`/`.git/info/exclude`, a real dot-directory, and a real ceiling — not a mock of any
//! of them. The pure ceiling/ignore-precedence *policy* that decides what to ask this port for is
//! covered in the core; this is where which paths the adapter itself hands back is pinned, since
//! that is the one decision this adapter makes.

use std::fs;
use std::path::Path;
use std::process::Command;

use soloist_core::filewatch::{Scan, ScanRequest, WatchScanner};
use soloist_sys::IgnoreWatchScanner;
use tempfile::TempDir;

/// A ceiling generous enough that no test tree here comes close to it, for every test but the one
/// that exercises the ceiling itself.
const GENEROUS_CEILING: usize = 10_000;

/// The ceiling `a_walk_past_the_ceiling_says_it_was_cut_short` deliberately sets below the tree it
/// scans.
const SMALL_CEILING: usize = 3;

/// The ceiling `files_are_reported_without_counting_against_the_ceiling` sets: exactly the two
/// directories of the tree it scans, and fewer than the files under them.
const TWO_DIRECTORIES: usize = 2;

/// How many directories a scan reported.
fn directories(scan: &Scan) -> usize {
    scan.paths
        .iter()
        .filter(|scanned| scanned.directory)
        .count()
}

/// How many non-directory paths a scan reported.
fn files(scan: &Scan) -> usize {
    scan.paths
        .iter()
        .filter(|scanned| !scanned.directory)
        .count()
}

/// A request scanning `root` with no name exclusions and the repository's own ignore rules
/// honoured — the ordinary case, overridden field by field where a test needs otherwise.
fn request(root: &Path) -> ScanRequest {
    ScanRequest {
        root: root.to_path_buf(),
        ignored_names: Vec::new(),
        honour_repository_ignores: true,
        ceiling: GENEROUS_CEILING,
    }
}

/// Whether any scanned path's final component is exactly `name`.
fn any_path_named(scan: &Scan, name: &str) -> bool {
    scan.paths
        .iter()
        .any(|scanned| scanned.path.ends_with(name))
}

/// A tempdir with `git init` already run in it, or `None` if this host has no `git` — the one
/// test needing a real repository skips itself with a clear message rather than pretending to
/// pass.
fn git_repository() -> Option<TempDir> {
    let dir = tempfile::tempdir().expect("temp dir");
    match Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
    {
        Ok(status) if status.success() => Some(dir),
        Ok(status) => panic!("git init exited with {status}"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => panic!("failed to run git: {err}"),
    }
}

#[test]
fn a_gitignored_directory_is_not_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::write(root.join(".gitignore"), "build/\n").expect("write .gitignore");
    fs::create_dir_all(root.join("build")).expect("create build dir");
    fs::write(root.join("build/output.txt"), "compiled").expect("write build output");
    fs::create_dir_all(root.join("src")).expect("create src dir");
    fs::write(root.join("src/main.rs"), "fn main() {}").expect("write src file");

    let scan = IgnoreWatchScanner::new().scan(request(root));

    assert!(
        !any_path_named(&scan, "build"),
        "a gitignored directory must not be reported: {:?}",
        scan.paths,
    );
    assert!(
        !any_path_named(&scan, "output.txt"),
        "a file beneath a gitignored directory must not be reported: {:?}",
        scan.paths,
    );
    assert!(
        any_path_named(&scan, "src"),
        "a directory the .gitignore does not name must still be reported: {:?}",
        scan.paths,
    );
}

#[test]
fn a_path_excluded_by_git_info_exclude_is_not_reported() {
    let Some(dir) = git_repository() else {
        eprintln!("skipping a_path_excluded_by_git_info_exclude_is_not_reported: git is not installed on this host");
        return;
    };
    let root = dir.path();
    fs::write(root.join(".git/info/exclude"), "worktrees/\n").expect("write git exclude");
    fs::create_dir_all(root.join("worktrees/one")).expect("create worktrees dir");
    fs::write(root.join("worktrees/one/file.txt"), "x").expect("write worktree file");

    let scan = IgnoreWatchScanner::new().scan(request(root));

    assert!(
        !any_path_named(&scan, "worktrees"),
        "a path named in .git/info/exclude must not be reported: {:?}",
        scan.paths,
    );
}

#[test]
fn a_tracked_dot_directory_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join(".github/workflows")).expect("create .github dir");
    fs::write(root.join(".github/workflows/ci.yml"), "name: ci").expect("write workflow file");

    let scan = IgnoreWatchScanner::new().scan(request(root));

    assert!(
        any_path_named(&scan, "workflows"),
        "a tracked dot-directory must still be reported — this is the case that fails if hidden(true) is left on: {:?}",
        scan.paths,
    );
}

#[test]
fn an_ignored_name_is_never_descended_into() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("node_modules/pkg/deep")).expect("create node_modules tree");
    fs::write(root.join("node_modules/pkg/deep/index.js"), "").expect("write nested file");

    let mut req = request(root);
    req.ignored_names = vec!["node_modules".to_string()];

    let scan = IgnoreWatchScanner::new().scan(req);

    assert!(
        !any_path_named(&scan, "node_modules"),
        "a name in ignored_names must not be reported, even with no .gitignore: {:?}",
        scan.paths,
    );
    assert!(
        !any_path_named(&scan, "deep"),
        "an ignored directory must not be descended into: {:?}",
        scan.paths,
    );
}

#[test]
fn repository_ignores_can_be_disabled() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::write(root.join(".gitignore"), "build/\n").expect("write .gitignore");
    fs::create_dir_all(root.join("build")).expect("create build dir");
    fs::write(root.join("build/config.json"), "{}").expect("write build file");

    let mut req = request(root);
    req.honour_repository_ignores = false;

    let scan = IgnoreWatchScanner::new().scan(req);

    assert!(
        any_path_named(&scan, "build"),
        "a gitignored directory a restart glob names explicitly must still be reported when \
         repository ignores are disabled: {:?}",
        scan.paths,
    );
    assert!(
        any_path_named(&scan, "config.json"),
        "a file beneath it must be reported too: {:?}",
        scan.paths,
    );
}

#[test]
fn a_walk_past_the_ceiling_says_it_was_cut_short() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    for name in ["a", "b", "c", "d", "e"] {
        fs::create_dir_all(root.join(name)).expect("create child dir");
    }

    let mut req = request(root);
    req.ceiling = SMALL_CEILING;

    let scan = IgnoreWatchScanner::new().scan(req);

    assert_eq!(
        directories(&scan),
        SMALL_CEILING,
        "a walk stops at its ceiling: {:?}",
        scan.paths,
    );
    assert!(
        scan.truncated,
        "a walk stopped short of the whole tree must say so",
    );
}

#[test]
fn files_are_reported_without_counting_against_the_ceiling() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).expect("create src dir");
    for name in ["a.rs", "b.rs", "c.rs", "d.rs"] {
        fs::write(root.join("src").join(name), "").expect("write source file");
    }

    let mut req = request(root);
    req.ceiling = TWO_DIRECTORIES;

    let scan = IgnoreWatchScanner::new().scan(req);

    assert!(
        !scan.truncated,
        "a tree whose directories all fit its ceiling was not cut short, however many files \
         they hold: {:?}",
        scan.paths,
    );
    assert!(
        any_path_named(&scan, "src"),
        "every directory within the ceiling is reported: {:?}",
        scan.paths,
    );
    assert!(
        files(&scan) <= TWO_DIRECTORIES,
        "the files reported alongside them stay bounded by the same ceiling: {:?}",
        scan.paths,
    );
}

#[test]
fn the_root_itself_is_reported_as_a_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();

    let scan = IgnoreWatchScanner::new().scan(request(root));

    let reported_root = scan
        .paths
        .iter()
        .find(|scanned| scanned.path.as_path() == root);
    assert!(
        reported_root.is_some_and(|scanned| scanned.directory),
        "the root itself must be reported as a directory: {:?}",
        scan.paths,
    );
}
