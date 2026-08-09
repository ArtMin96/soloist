//! What an agent is asked when it is asked to draft a commit message.
//!
//! **A diff says what changed and never why.** So the change arrives with what context is cheap and
//! honest: the branch it is on, the task somebody set out to do, and the subjects of the
//! repository's own recent commits — which is the only place the house's way of writing one is
//! written down. The examples are examples of *form*, stated as such, never material to copy.
//!
//! **The shape is the repository's to set.** A `commit.template` is what this repository tells
//! everybody a message should look like, so where there is one it goes in last, under its own
//! label, and what is asked for is that it be filled in rather than replaced with something the
//! agent liked better. Where there is none, the form of the answer is stated instead.
//!
//! **There is a ceiling, and it falls on a whole path.** A prompt is composed to fit
//! [`ONE_SHOT_PROMPT_LIMIT`] rather than cut to it, and the parts are budgeted in the order they
//! matter: what is being asked, then the context, then as much of the change as is left. Past the
//! ceiling every path is described by name and by what happened to it instead.
//!
//! **The draft is advisory.** Nothing here or above it commits: the text goes back to whoever asked,
//! to read and change first. That is why the composition can afford to leave things out.

use std::path::Path;

use crate::agents::ONE_SHOT_PROMPT_LIMIT;
use crate::ids::ProjectId;
use crate::vcs::{BranchInfo, CommitEntry, FileChange};

use super::error::{GitDraftError, GitError};
use super::message_change::{describes_intent, Description};
use super::repository::LogRange;
use super::status::Git;

/// How many recent commit subjects are shown as examples of how this repository writes one.
const VOICE_EXAMPLES: usize = 10;

/// How many recent commits are looked at to find that many. More than [`VOICE_EXAMPLES`], because
/// the ones nobody authored are passed over and a run of merges would otherwise leave none.
const VOICE_EXAMPLE_SCAN: usize = 30;

/// The prefix version control gives a revert's subject, which is another commit's subject quoted.
/// Not anybody's writing, so it teaches nothing about the house voice.
const REVERT_PREFIX: &str = "Revert \"";

/// What marks an author as a program rather than a person, by the forge convention that appends it
/// to a machine account's name. A commit nobody wrote is not an example of how anybody writes.
const BOT_SUFFIX: &str = "[bot]";

/// The most of a task's title that is carried. Long enough for any title somebody writes as one,
/// bounded because a title is a line and what arrives here was typed by an agent.
const INTENT_TITLE_LIMIT: usize = 256;

/// The most of a task's own description that is carried. A task body is free-form and can run to
/// pages of specification, while what a commit message needs from it is what it says first.
const INTENT_BODY_LIMIT: usize = 2 * 1024;

/// What the agent is asked to do, ahead of a change described by its diffs.
const PATCH_INSTRUCTIONS: &str = "\
Write a git commit message for the staged change below.

";

/// The same, ahead of a change too large to show, described by its paths.
const SUMMARY_INSTRUCTIONS: &str = "\
Write a git commit message for the staged change below.

The change is too large to include, so only the files it touches are listed. Describe the change at
that level rather than inventing detail about the contents.

";

/// What form the answer takes where this repository asks for none of its own.
const OWN_FORM: &str = "\
Reply with the message and nothing else: no preamble, no explanation, no code fence. Use a short
imperative subject line of at most 72 characters. Add a body only if the change needs one, separated
from the subject by a blank line, wrapped at 72 characters, saying why rather than restating the
diff.

";

/// What form the answer takes where the repository configures one. Filling a shape somebody
/// committed is what honouring it means; writing something like it is not.
const SKELETON_FORM: &str = "\
Fill in the template at the end of this message rather than writing a message of your own shape.
Keep its headings, their order, and any checklist it carries exactly as they are, and replace only
the parts meant to be filled in. Leave a section out only if it plainly does not apply. Reply with
the message and nothing else: no preamble, no explanation, no code fence.

";

/// The line that introduces the change itself, in each of the two forms it can take, and the one
/// that introduces the shape to fill. Everything is under a label of its own, so no part of the
/// prompt can be read as another part.
const PATCH_LABEL: &str = "Staged change:\n";
const PATHS_LABEL: &str = "Staged files:\n";
const SKELETON_LABEL: &str = "\nTemplate to fill in:\n";

/// The most the fixed text around a description can cost. The choice between the two forms is made
/// after the budget is set, so the budget has to hold for whichever is chosen.
const INSTRUCTIONS_HEADROOM: usize = longest(
    PATCH_INSTRUCTIONS.len() + PATCH_LABEL.len(),
    SUMMARY_INSTRUCTIONS.len() + PATHS_LABEL.len(),
);

const fn longest(one: usize, other: usize) -> usize {
    if one > other {
        one
    } else {
        other
    }
}

/// How the recent subjects are introduced. It says what they are for, and what they are not for:
/// a model handed example subjects beside an unrelated diff will otherwise reach for one.
const VOICE_PREAMBLE: &str = "\
For style only, here is how this repository writes commit subjects. Match their voice and length.
Do not reuse any of them and do not describe what they describe — they are unrelated to the change
below.

";

/// How the intent is introduced. It says where it came from and what outranks it, because a task is
/// what somebody set out to do and the diff is what they did — and it is the diff being committed.
const INTENT_PREAMBLE: &str = "\
What this work set out to do, from the task the agent making the change is working on. Use it to say
why the change was made. Where it and the change disagree, the change is what is being committed:
describe only what the change below actually does.

";

/// What a change was for, in the words of whoever set out to make it.
///
/// Composed by whoever knows which work is in flight — this context does not, and must not: what a
/// project's agents are working on is coordination's fact, not version control's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitIntent {
    /// What the task is called.
    pub title: String,
    /// What the task says to do, free-form and possibly long; only its opening is carried.
    pub body: String,
}

impl Git {
    /// The prompt that asks for a commit message describing what is staged in `project`, saying
    /// what the change was for when `intent` names it.
    ///
    /// Gated on the user having trusted the project, for the same reason a change to it is: what
    /// runs is an agent CLI with the project as its working directory, and an agent CLI reads the
    /// project's own configuration — so it runs code the project carries.
    ///
    /// [`GitDraftError::NothingToDescribe`] when nothing staged says anything: an empty index, a
    /// root that is not a repository, or a change made only of files a tool wrote. That refusal is
    /// reached from the remembered status alone, so it costs no subprocess.
    ///
    /// Reads the repository, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub fn commit_message_prompt(
        &self,
        project: ProjectId,
        root: &Path,
        intent: Option<&CommitIntent>,
    ) -> Result<String, GitDraftError> {
        if !self.trusted(project)? {
            return Err(GitDraftError::Untrusted);
        }
        let Some(status) = self.status(project, root)? else {
            return Err(GitDraftError::NothingToDescribe);
        };
        let staged: Vec<&FileChange> = status
            .changes
            .iter()
            .filter(|change| change.status.staged.is_some() && describes_intent(&change.path))
            .collect();
        if staged.is_empty() {
            return Err(GitDraftError::NothingToDescribe);
        }

        let context = describe_branch(&status.branch);
        let purpose = intent.map(describe_intent).unwrap_or_default();
        let voice = self.voice(project, root)?;
        let shape = match self.configured_template(project, root)? {
            Some(template) => format!("{SKELETON_LABEL}{template}"),
            None => String::new(),
        };
        let form = if shape.is_empty() {
            OWN_FORM
        } else {
            SKELETON_FORM
        };
        let budget = ONE_SHOT_PROMPT_LIMIT.saturating_sub(
            INSTRUCTIONS_HEADROOM
                + form.len()
                + context.len()
                + purpose.len()
                + voice.len()
                + shape.len(),
        );
        // What is being asked, then the context it is being asked in, then the change — so nothing
        // ahead of the change's label can be mistaken for part of it, and only the shape to fill
        // follows it, under a label of its own.
        Ok(match self.describe(project, root, &staged, budget)? {
            Description::Patches(patches) => format!(
                "{PATCH_INSTRUCTIONS}{form}{context}{purpose}{voice}{PATCH_LABEL}{patches}{shape}"
            ),
            Description::Paths(paths) => format!(
                "{SUMMARY_INSTRUCTIONS}{form}{context}{purpose}{voice}{PATHS_LABEL}{paths}{shape}"
            ),
        })
    }

    /// The block of recent subjects that shows how this repository writes one, or nothing at all
    /// when there are none to show.
    ///
    /// Empty is an ordinary answer, not a failure: a repository with no commits yet, a first commit
    /// on an orphan branch, and a clone shallow enough to hold none all reach it — and a prompt
    /// without examples still asks the same question.
    fn voice(&self, project: ProjectId, root: &Path) -> Result<String, GitError> {
        let Some(recent) =
            self.history(project, root, LogRange::CheckedOut, 0, VOICE_EXAMPLE_SCAN)?
        else {
            return Ok(String::new());
        };
        let mut block = String::new();
        for subject in recent
            .iter()
            .filter(|commit| is_authored(commit))
            .map(|commit| commit.subject.as_str())
            .take(VOICE_EXAMPLES)
        {
            if block.is_empty() {
                block.push_str(VOICE_PREAMBLE);
            }
            block.push_str("- ");
            block.push_str(subject);
            block.push('\n');
        }
        if !block.is_empty() {
            block.push('\n');
        }
        Ok(block)
    }
}

/// Where the change is being made, in one line. Cheap — the status already carries it — and worth
/// saying, because a branch is often the only place its purpose is named.
fn describe_branch(branch: &BranchInfo) -> String {
    match &branch.name {
        Some(name) => format!("On branch {name}.\n\n"),
        None => String::new(),
    }
}

/// What the change was for, bounded: the task's title, and as much of what it says as is worth
/// carrying beside a diff.
fn describe_intent(intent: &CommitIntent) -> String {
    let mut block = String::from(INTENT_PREAMBLE);
    block.push_str(excerpt(intent.title.trim(), INTENT_TITLE_LIMIT));
    block.push('\n');
    let body = excerpt(intent.body.trim(), INTENT_BODY_LIMIT);
    if !body.is_empty() {
        block.push('\n');
        block.push_str(body);
        block.push('\n');
    }
    block.push('\n');
    block
}

/// The opening of `text` that fits `limit`, ending at a line boundary where there is one — so what
/// arrives reads as the beginning of what somebody wrote rather than a sentence stopped mid-word.
fn excerpt(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    // Slicing anywhere but a character boundary is not a string; the limit is a byte count, so the
    // last boundary at or before it is where the cut can be made.
    let boundary = (0..=limit)
        .rev()
        .find(|at| text.is_char_boundary(*at))
        .unwrap_or(0);
    let head = &text[..boundary];
    match head.rfind('\n') {
        Some(line_end) => &text[..line_end],
        None => head,
    }
}

/// Whether a commit is an example of how somebody here writes one.
///
/// Three are not: a merge records no change anyone authored, a revert's subject is another commit's
/// quoted back, and a commit a program made is nobody's writing.
fn is_authored(commit: &CommitEntry) -> bool {
    !commit.merge
        && !commit.subject.starts_with(REVERT_PREFIX)
        && !commit.author.ends_with(BOT_SUFFIX)
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
