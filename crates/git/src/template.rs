//! The message a new commit starts from, as the repository's own configuration supplies it.
//!
//! `commit.template` names a file, and version control reads it only when it is about to open an
//! editor. Soloist commits with the message it was given, so it never would — which is why the
//! template is resolved here instead and offered as the starting text of the message box, where
//! the person writing the commit can read the hints and replace them.
//!
//! Three things about that resolution are version control's own rules rather than this adapter's
//! guesses, and each is answered by asking it: the value is read as a **pathname**, so `~/` is
//! expanded the way `git-config(1)` documents; a relative one is resolved against the top of the
//! working tree, which is where a commit runs from; and the guidance lines are removed by
//! `git stripspace`, which knows the repository's own comment character. Nothing here parses a
//! sentence — the one status that is read is the number version control exits with when a
//! configuration key is simply not set.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use soloist_core::GitError;

use crate::runner::{self, Run};

/// Asks for the configured template as a pathname, so the tilde expansion `git-config(1)`
/// documents for that value type is version control's rather than this adapter's.
const CONFIG_ARGS: &[&str] = &["config", "--get", "--type=path", "commit.template"];

/// The status version control exits with when the key asked for is not set. Machine data, and the
/// only reading of a failure here: every other status is a genuine failure to resolve, which is
/// a different thing from a repository that configures no template.
const NOT_SET: i32 = 1;

/// Where a relative template is resolved from — the top of the working tree, because that is
/// where a commit itself runs, measured rather than assumed.
const TOPLEVEL_ARGS: &[&str] = &["rev-parse", "--show-toplevel"];

/// Removes the guidance lines and the blank space around them, exactly as version control does to
/// a message that was edited. It reads the repository's own comment character, which is why the
/// question is asked rather than answered here.
const STRIP_ARGS: &[&str] = &["stripspace", "--strip-comments"];

/// The template the repository at `root` starts a commit message from, within `limit`.
pub(crate) fn commit_template(root: &Path, limit: usize) -> Result<Option<String>, GitError> {
    let Some(configured) = configured(root)? else {
        return Ok(None);
    };
    let Some(text) = read_within(&resolve(root, &configured)?, limit) else {
        return Ok(None);
    };
    let stripped = runner::run_with(
        root,
        STRIP_ARGS,
        Run {
            input: Some(&text),
            ..Run::default()
        },
    )?;
    let template = String::from_utf8_lossy(&stripped).into_owned();
    Ok((!template.is_empty()).then_some(template))
}

/// What the repository's configuration names as a template, `None` where it names nothing.
fn configured(root: &Path) -> Result<Option<PathBuf>, GitError> {
    let named = match runner::run(root, CONFIG_ARGS) {
        Ok(output) => output,
        Err(GitError::Op {
            status: Some(NOT_SET),
        }) => return Ok(None),
        Err(err) => return Err(err),
    };
    let named = String::from_utf8_lossy(&named).trim().to_string();
    Ok((!named.is_empty()).then(|| PathBuf::from(named)))
}

/// Where `configured` actually is: resolved against the top of the working tree rather than against
/// the folder the project was opened at, which is only the same path when the project *is* the
/// repository.
///
/// An absolute path needs no resolving and takes this route anyway, because joining onto one
/// replaces the base — so the top is asked for and then discarded, which is one short local
/// invocation rather than a branch nothing could observe.
fn resolve(root: &Path, configured: &Path) -> Result<PathBuf, GitError> {
    let toplevel = runner::run(root, TOPLEVEL_ARGS)?;
    let toplevel = String::from_utf8_lossy(&toplevel).trim().to_string();
    Ok(Path::new(&toplevel).join(configured))
}

/// The file's text when it is text and fits within `limit`, `None` otherwise — a template that is
/// not there, is not readable, is not text, or is longer than a message box would hold is a
/// message with no template rather than a failure to report.
fn read_within(path: &Path, limit: usize) -> Option<String> {
    let file = File::open(path).ok()?;
    // One byte past the ceiling is all it takes to know the ceiling was crossed.
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > limit {
        return None;
    }
    String::from_utf8(bytes).ok()
}
