//! The version-control adapter for Soloist (a driven adapter crate).
//!
//! Implements the core's [`GitRepository`] port against the system `git` command line, so the
//! pure core never runs a subprocess itself. Driving the user's own `git` is what makes their
//! hooks, their signing, and their configuration apply by construction, and it is why Soloist
//! stores no credential of its own.
//!
//! The crate depends only on `soloist-core` and the operating system — never the reverse (the
//! dependency-direction guard enforces it).

mod files_parse;
mod runner;
mod status_parse;

use std::path::Path;

use soloist_core::{GitError, GitRepository, GitStatus, ProjectFile};

/// The arguments asking for a working tree's state in the one machine-readable form this
/// adapter reads.
const STATUS_ARGS: &[&str] = &["status", "--porcelain=v2", "-z", "--branch"];

/// The arguments listing every path the repository tracks or has not been told to ignore.
const LISTED_ARGS: &[&str] = &[
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
];

/// The arguments listing what the repository was told to ignore. `--directory` stops an ignored
/// folder from being walked: it is reported as itself, which is what keeps a working tree
/// carrying build output from listing its every file.
const IGNORED_ARGS: &[&str] = &[
    "ls-files",
    "-z",
    "--others",
    "--ignored",
    "--exclude-standard",
    "--directory",
];

/// The question that tells a folder outside any repository apart from an invocation that
/// failed for some other reason. Asked only when something did fail, and answered with a fixed
/// word rather than a sentence — so no translated diagnostic is ever read.
const INSIDE_WORK_TREE_ARGS: &[&str] = &["rev-parse", "--is-inside-work-tree"];

/// What [`INSIDE_WORK_TREE_ARGS`] prints for a path inside a working tree.
const INSIDE_WORK_TREE: &[u8] = b"true";

/// Reads working trees by running the system `git` command line.
#[derive(Clone, Copy, Default)]
pub struct CliGitRepository;

impl CliGitRepository {
    /// A repository reader over whichever `git` the user has installed.
    pub fn new() -> Self {
        Self
    }
}

impl GitRepository for CliGitRepository {
    fn status(&self, root: &Path) -> Result<GitStatus, GitError> {
        let output = match runner::run(root, STATUS_ARGS) {
            Ok(output) => output,
            // A folder simply not being under version control is an ordinary state rather than
            // a fault, but the failure that says so is prose. Ask a question with a yes-or-no
            // answer instead of reading it.
            Err(GitError::Op { .. }) if !inside_work_tree(root) => return Err(GitError::NotARepo),
            Err(err) => return Err(err),
        };
        status_parse::parse(&output).ok_or(GitError::Op { status: None })
    }

    fn list_files(&self, root: &Path) -> Result<Vec<ProjectFile>, GitError> {
        // Two invocations because one cannot answer both: asking for ignored paths replaces the
        // listing rather than adding to it. The listed half runs first, so a folder outside any
        // repository is recognised before the second is spent.
        let listed = match runner::run(root, LISTED_ARGS) {
            Ok(output) => output,
            Err(GitError::Op { .. }) if !inside_work_tree(root) => return Err(GitError::NotARepo),
            Err(err) => return Err(err),
        };
        let ignored = runner::run(root, IGNORED_ARGS)?;
        Ok(files_parse::parse(&listed, false)
            .chain(files_parse::parse(&ignored, true))
            .collect())
    }
}

/// Whether `root` is inside a working tree.
fn inside_work_tree(root: &Path) -> bool {
    runner::run(root, INSIDE_WORK_TREE_ARGS)
        .is_ok_and(|answer| answer.trim_ascii() == INSIDE_WORK_TREE)
}
