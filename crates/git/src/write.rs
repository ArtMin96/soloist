//! The invocations that change a working tree.
//!
//! Whole-path moves are handed straight to version control, which is the point of driving the
//! command line: `add`, `restore`, and `commit` are its own operations, so the user's
//! configuration, hooks and signing key apply without a line of code here.
//!
//! A single hunk is the same move with a patch attached. The patch is version control's own
//! bytes, kept whole and unedited (see [`crate::patch`]), fed back over standard input — so a
//! carriage return, a file that ends without a newline, and a path that moved are all carried
//! exactly as they were produced rather than rebuilt from an understanding of them.

use std::path::Path;

use soloist_core::{GitError, HunkRange, RawFileDiff};

use crate::patch;
use crate::runner::{self, Run};

/// The separator after which every remaining argument is a path, so a file named like an option
/// is still read as a file.
const PATHS: &str = "--";

/// Which copy of a file one hunk is applied to, and which way round.
pub(crate) enum Apply {
    /// Record it in the index, leaving the working tree as it is.
    Stage,
    /// Take it back out of the index, leaving the working tree as it is.
    Unstage,
    /// Undo it in the working tree, leaving the index as it is.
    Discard,
}

impl Apply {
    /// The arguments that place a patch where this asks for it.
    fn args(&self) -> &'static [&'static str] {
        match self {
            Apply::Stage => &["apply", "--cached"],
            Apply::Unstage => &["apply", "--cached", "--reverse"],
            Apply::Discard => &["apply", "--reverse"],
        }
    }
}

/// Records everything the working tree holds for `path` in the index. A path that is gone has
/// its absence recorded, which is the same operation.
pub(crate) fn stage(root: &Path, path: &str, original_path: Option<&str>) -> Result<(), GitError> {
    run(root, &["add", PATHS], path, original_path)
}

/// Takes `path` back out of the index, leaving the working tree alone.
pub(crate) fn unstage(
    root: &Path,
    path: &str,
    original_path: Option<&str>,
) -> Result<(), GitError> {
    run(root, &["restore", "--staged", PATHS], path, original_path)
}

/// Restores `path` in the working tree from the index, which throws away everything the working
/// tree held beyond it — and nothing more, because the index is as far back as it reaches.
pub(crate) fn discard(root: &Path, path: &str) -> Result<(), GitError> {
    run(root, &["restore", PATHS], path, None)
}

/// Applies the one hunk of `diff` that falls at `hunk`, where `apply` asks for it.
///
/// [`GitError::HunkGone`] when the diff no longer holds a hunk there: the file moved on between
/// being read and being acted on, and there is nothing this could do that the caller asked for.
pub(crate) fn apply_hunk(
    root: &Path,
    diff: &RawFileDiff,
    hunk: HunkRange,
    apply: Apply,
) -> Result<(), GitError> {
    let patch = patch::one_hunk(diff, hunk).ok_or(GitError::HunkGone)?;
    runner::run_with(
        root,
        apply.args(),
        Run {
            input: Some(&patch),
            ..Run::default()
        },
    )
    .map(|_| ())
}

/// Records the index as a commit carrying `message`, or replaces the last commit with it.
///
/// This is the invocation that runs the user's own code — their `pre-commit` and `commit-msg`
/// hooks — so it is the one whose account of itself is carried back when it is refused. That
/// text is the hook's, not version control's, and showing it is the only way a rejected commit
/// says anything useful.
pub(crate) fn commit(root: &Path, message: &str, amend: bool) -> Result<(), GitError> {
    let mut args = vec!["commit"];
    if amend {
        args.push("--amend");
    }
    args.extend_from_slice(&["--message", message]);
    runner::run_with(
        root,
        &args,
        Run {
            report_refusal: true,
            ..Run::default()
        },
    )
    .map(|_| ())
}

/// Runs one whole-path invocation. A renamed path is named by both of its names, because given
/// one version control sees a file deleted and an unrelated one appear, and records half a move.
fn run(
    root: &Path,
    command: &[&str],
    path: &str,
    original_path: Option<&str>,
) -> Result<(), GitError> {
    let mut args = command.to_vec();
    args.push(path);
    args.extend(original_path);
    runner::run(root, &args).map(|_| ())
}
