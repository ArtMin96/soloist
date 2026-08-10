//! The title and description a pull request would be opened with, computed from what the branch
//! proposes — so proposing one is a button rather than a form.
//!
//! **The branch's commits are the answer.** A branch carrying one commit has already been described
//! once, by whoever wrote it: its subject is the title and the rest of its message the description,
//! and writing either again by hand is copying. A branch carrying several has no single account of
//! itself, so its name stands in for the title — a name is what its author called the work — and the
//! subjects stand in for the description.
//!
//! **A repository's own shape still wins.** Where a description template is on offer, what the
//! branch says is written *into* it rather than in place of it: every heading and checklist the
//! repository asked for survives, which is the same contract the drafted description honours.
//!
//! **Nothing here proposes anything.** What comes back is text for a box somebody looks at before
//! they press the button, and a suggestion that comes back blank is refused by the same guard that
//! refuses a blank title typed by hand.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ids::ProjectId;

use super::pr::PullRequestError;
use super::proposed::commit_lines;
use super::skeleton::shape;
use super::status::Git;

/// Leading branch-name segments that name a kind of work rather than the work itself. A title says
/// what changed; that it was a fix rather than a feature is what the pull request itself is for.
const KIND_PREFIXES: [&str; 14] = [
    "feat", "feature", "fix", "bugfix", "hotfix", "chore", "docs", "refactor", "perf", "test",
    "style", "build", "ci", "revert",
];

/// What a branch name separates its words with.
const WORD_MARKS: [char; 2] = ['-', '_'];

/// The other thing a branch name separates a segment with: the path mark, which names a namespace
/// rather than a word.
const PATH_MARK: char = '/';

/// What marks a line of a shape as one of its headings, and so as the place the branch's own account
/// of itself belongs.
const HEADING: char = '#';

/// The most of the branch's account of itself that is carried into a description: fifty commit
/// subjects, or one commit's body, with room over. Prose past it is left out whole rather than cut —
/// the same choice a template past its own ceiling gets, and for the same reason.
const FILLING_LIMIT: usize = 16 * 1024;

/// The most of a title that is carried. A title is one line by nature, but a commit's subject is
/// whatever version control folded its first paragraph into — a paragraph, where its author wrote no
/// blank line after the first line — so what arrives here can be prose of any length.
///
/// Its opening is kept rather than dropped, unlike a description past [`FILLING_LIMIT`]: a
/// description is optional and a title is the thing the button exists to fill in, so coming back
/// blank would turn one click into two.
const TITLE_LIMIT: usize = 256;

/// What a pull request would be opened with, before anybody has typed anything.
///
/// Both fields are the caller's to change and neither is a promise: a title computed from a commit
/// that says nothing comes back blank, and the proposal itself refuses it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PullRequestSuggestion {
    /// The one-line title: the single commit's subject, or the branch's name read as a sentence.
    pub title: String,
    /// The description: what the branch says about itself, written into the shape on offer when
    /// there was one worth writing into.
    pub body: String,
}

impl Git {
    /// What `project`'s checked-out branch would be proposed as against `base`, filling `skeleton`
    /// when there is one on offer.
    ///
    /// A read, and ungated unlike the drafted description: nothing the repository configures is run
    /// here, only the log the working tree already carries.
    ///
    /// Refuses exactly as [`Git::proposed`] does — most importantly with
    /// [`PullRequestError::NothingToDescribe`] for a branch holding nothing its base does not, which
    /// is the one state where no title could be computed rather than one being invented.
    ///
    /// Reads the repository, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub(super) fn pull_request_suggestion(
        &self,
        project: ProjectId,
        root: &Path,
        base: &str,
        skeleton: &str,
    ) -> Result<PullRequestSuggestion, PullRequestError> {
        let proposed = self.proposed(project, root, base)?;
        let (title, filling) = match proposed.commits.as_slice() {
            [only] => (only.subject.clone(), fitting(&only.body)),
            commits => (
                humanized(&proposed.head),
                commit_lines(commits, FILLING_LIMIT),
            ),
        };
        Ok(PullRequestSuggestion {
            title: opening(&title),
            body: filled(skeleton, &filling),
        })
    }
}

/// The opening of `title` that fits [`TITLE_LIMIT`], cut at a word boundary where there is one — so
/// what comes back reads as the beginning of what somebody wrote rather than a word broken in half.
fn opening(title: &str) -> String {
    if title.len() <= TITLE_LIMIT {
        return title.to_string();
    }
    // Slicing anywhere but a character boundary is not a string; the limit is a byte count, so the
    // last boundary at or before it is where the cut can be made.
    let boundary = (0..=TITLE_LIMIT)
        .rev()
        .find(|at| title.is_char_boundary(*at))
        .unwrap_or(0);
    let head = &title[..boundary];
    match head.rfind(char::is_whitespace) {
        Some(word_ends) => head[..word_ends].trim_end().to_string(),
        None => head.to_string(),
    }
}

/// One commit's body where it fits the ceiling, and nothing at all where it does not.
fn fitting(body: &str) -> String {
    if body.len() <= FILLING_LIMIT {
        body.to_string()
    } else {
        String::new()
    }
}

/// The description: the shape with what the branch says written into it, or what the branch says
/// alone where there is no shape worth filling.
///
/// It goes under the shape's first heading, which is where a template asks what changed; a shape
/// carrying no heading takes it above whatever it does carry. Either way every line of the shape
/// survives — a repository carrying a template is telling everybody who opens a pull request what it
/// expects to read, and filling one in is not rewriting it.
fn filled(skeleton: &str, filling: &str) -> String {
    let filling = filling.trim();
    let Some(shape) = shape(skeleton) else {
        return filling.to_string();
    };
    if filling.is_empty() {
        return shape.to_string();
    }
    match heading_ends(shape) {
        Some(at) => format!("{}\n\n{filling}\n{}", shape[..at].trim_end(), &shape[at..]),
        None => format!("{filling}\n\n{shape}"),
    }
}

/// Where the shape's first heading line ends, or `None` where it carries no heading.
fn heading_ends(shape: &str) -> Option<usize> {
    let mut at = 0;
    for line in shape.split_inclusive('\n') {
        if line.trim_start().starts_with(HEADING) {
            return Some(at + line.len());
        }
        at += line.len();
    }
    None
}

/// A branch name read as a sentence: the kind-of-work segment dropped, the separators read as
/// spaces, and the first letter raised. `feat/live-changes-rail` becomes `Live changes rail`; a name
/// already written as a sentence keeps every capital its author chose.
fn humanized(branch: &str) -> String {
    let words: Vec<&str> = without_kind(branch.trim())
        .split(WORD_MARKS)
        .filter(|word| !word.is_empty())
        .collect();
    capitalized(&words.join(" "))
}

/// `branch` with a leading kind-of-work segment dropped, or the whole of it where it carries none.
fn without_kind(branch: &str) -> &str {
    KIND_PREFIXES
        .iter()
        .find_map(|prefix| stripped_kind(branch, prefix))
        .unwrap_or(branch)
}

/// What follows `prefix` and the mark after it in `branch`, or `None` where `branch` does not begin
/// with that segment. The comparison ignores case, since a branch name is typed by hand.
fn stripped_kind<'a>(branch: &'a str, prefix: &str) -> Option<&'a str> {
    if !branch.get(..prefix.len())?.eq_ignore_ascii_case(prefix) {
        return None;
    }
    let mut rest = branch[prefix.len()..].chars();
    separates_segments(rest.next()?).then_some(rest.as_str())
}

/// Whether `mark` separates a leading kind-of-work segment from the name behind it: the path mark, or
/// any mark the name separates its words with.
///
/// One rule for all of them, because `fix/login`, `fix-login`, and `fix_login` are three spellings of
/// the same thing — a name whose author said what kind of work it was first — and a title computed
/// from them should not depend on which spelling they picked.
fn separates_segments(mark: char) -> bool {
    mark == PATH_MARK || WORD_MARKS.contains(&mark)
}

/// `sentence` with its first letter raised and the rest left exactly as it was.
fn capitalized(sentence: &str) -> String {
    let mut letters = sentence.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "suggestion_tests.rs"]
mod tests;
