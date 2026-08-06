//! The version-control adapter for Soloist (a driven adapter crate).
//!
//! Implements the core's [`GitRepository`] port against the system `git` command line, so the
//! pure core never runs a subprocess itself. Driving the user's own `git` is what makes their
//! hooks, their signing, and their configuration apply by construction, and it is why Soloist
//! stores no credential of its own.
//!
//! The crate depends only on `soloist-core` and the operating system — never the reverse (the
//! dependency-direction guard enforces it).

mod diff_parse;
mod files_parse;
mod patch;
mod runner;
mod status_parse;
mod write;

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use soloist_core::{
    DiffTarget, FileContent, GitError, GitRepository, GitStatus, HunkRange, ProjectFile,
    RawFileDiff,
};

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

/// The arguments that make a diff say the same thing on every machine: counted form first (the
/// only place "binary" is stated as data), then the patch, with every part of the output a
/// user's own configuration could otherwise have changed pinned back to its default.
const DIFF_ARGS: &[&str] = &[
    "--numstat",
    "--patch",
    "--no-color",
    "--no-ext-diff",
    "--find-renames",
    "--src-prefix=a/",
    "--dst-prefix=b/",
];

/// What is compared against a path version control does not track: nothing at all.
const NOTHING: &str = "/dev/null";

/// The status a comparison against something outside the repository reports when the two sides
/// differ. It is an answer, not a failure — the only exit this adapter accepts as one.
const DIFFERED: i32 = 1;

/// The most of one file this adapter carries. A file past it arrives cut, and says so; reading
/// a pipe or a path without a ceiling is how one pathological file becomes an out-of-memory.
const FILE_LIMIT: usize = 1024 * 1024;

/// How far into a file the byte that means "not text" is looked for, matching what version
/// control itself inspects.
const BINARY_SNIFF: usize = 8000;

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

    fn diff(
        &self,
        root: &Path,
        target: DiffTarget,
        path: &str,
        original_path: Option<&str>,
    ) -> Result<RawFileDiff, GitError> {
        let untracked = matches!(target, DiffTarget::Untracked);
        let mut args = vec!["diff"];
        match target {
            DiffTarget::Staged => args.push("--cached"),
            DiffTarget::Unstaged => {}
            DiffTarget::Head => args.push("HEAD"),
            // Nothing in the repository to compare against, so the comparison is made against
            // a path outside it — which is also why this is the one invocation whose non-zero
            // exit is an answer.
            DiffTarget::Untracked => args.push("--no-index"),
        }
        args.extend_from_slice(DIFF_ARGS);
        args.push("--");
        if untracked {
            args.push(NOTHING);
        }
        args.push(path);
        // A rename is recognised only when both of its names are asked about together: given
        // one, version control sees a file deleted and an unrelated one added.
        args.extend(original_path);

        let accepted = untracked.then_some(DIFFERED);
        let output = match runner::run_accepting(root, &args, accepted) {
            Ok(output) => output,
            Err(GitError::Op { .. }) if !inside_work_tree(root) => return Err(GitError::NotARepo),
            Err(err) => return Err(err),
        };
        Ok(diff_parse::parse(&output))
    }

    fn read_file(&self, root: &Path, path: &str) -> Result<Option<FileContent>, GitError> {
        let file = match File::open(root.join(path)) {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(GitError::Op { status: None }),
        };
        // A listing carries directories as well as files, so being handed one is ordinary
        // rather than a fault — and there is no content in it to show.
        if file.metadata().is_ok_and(|metadata| metadata.is_dir()) {
            return Ok(None);
        }
        // One byte past the ceiling is all it takes to know the ceiling was crossed.
        let mut bytes = Vec::new();
        file.take(FILE_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| GitError::Op { status: None })?;
        Ok(Some(content(bytes)))
    }

    fn stage(&self, root: &Path, path: &str, original_path: Option<&str>) -> Result<(), GitError> {
        write::stage(root, path, original_path)
    }

    fn unstage(
        &self,
        root: &Path,
        path: &str,
        original_path: Option<&str>,
    ) -> Result<(), GitError> {
        write::unstage(root, path, original_path)
    }

    fn discard(&self, root: &Path, path: &str) -> Result<(), GitError> {
        write::discard(root, path)
    }

    fn stage_hunk(
        &self,
        root: &Path,
        path: &str,
        original_path: Option<&str>,
        hunk: HunkRange,
    ) -> Result<(), GitError> {
        let diff = self.diff(root, DiffTarget::Unstaged, path, original_path)?;
        write::apply_hunk(root, &diff, hunk, write::Apply::Stage)
    }

    fn unstage_hunk(
        &self,
        root: &Path,
        path: &str,
        original_path: Option<&str>,
        hunk: HunkRange,
    ) -> Result<(), GitError> {
        let diff = self.diff(root, DiffTarget::Staged, path, original_path)?;
        write::apply_hunk(root, &diff, hunk, write::Apply::Unstage)
    }

    fn discard_hunk(&self, root: &Path, path: &str, hunk: HunkRange) -> Result<(), GitError> {
        let diff = self.diff(root, DiffTarget::Unstaged, path, None)?;
        write::apply_hunk(root, &diff, hunk, write::Apply::Discard)
    }

    fn commit(&self, root: &Path, message: &str, amend: bool) -> Result<(), GitError> {
        write::commit(root, message, amend)
    }
}

/// One file's bytes as a reader is given them: cut at the ceiling if it was over it, and with
/// no text at all when they are not text.
fn content(mut bytes: Vec<u8>) -> FileContent {
    let truncated = bytes.len() > FILE_LIMIT;
    bytes.truncate(FILE_LIMIT);
    if bytes.iter().take(BINARY_SNIFF).any(|&byte| byte == 0) {
        return FileContent {
            text: None,
            truncated,
        };
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => Some(text),
        // A read cut at the ceiling can end part-way through a character. That is the cut
        // showing, not the file being unreadable, so only the last character is lost.
        Err(err) if truncated => Some(String::from_utf8_lossy(err.as_bytes()).into_owned()),
        Err(_) => None,
    };
    FileContent { text, truncated }
}

/// Whether `root` is inside a working tree.
fn inside_work_tree(root: &Path) -> bool {
    runner::run(root, INSIDE_WORK_TREE_ARGS)
        .is_ok_and(|answer| answer.trim_ascii() == INSIDE_WORK_TREE)
}
