//! What an agent is told about a branch when it is asked to describe it as a pull request.
//!
//! **The skeleton is the contract.** A repository that carries a pull-request template is telling
//! everybody who opens one what it expects to read, and an agent that answers in its own shape
//! instead has ignored the house. So the template goes in **last**, under its own label, after
//! everything it is to be filled from — and the instructions say to keep its headings rather than
//! to write something like them.
//!
//! **What a branch proposes is its commits, not its diff.** They are already somebody's account of
//! their own work, they are short, and they are in order, so they say more per byte than any patch
//! could.
//!
//! **There is a ceiling.** The prompt is composed to [`ONE_SHOT_PROMPT_LIMIT`] rather than cut to
//! it. A template past the shape ceiling is dropped whole and a plain description asked for
//! instead, because half a skeleton would be filled in as if it were the whole of one.
//!
//! **The draft is advisory.** Nothing here or above it proposes anything: the text goes back to
//! whoever asked, to read and change before they ever press the button.

use std::path::Path;

use crate::agents::ONE_SHOT_PROMPT_LIMIT;
use crate::ids::ProjectId;

use super::pr::PullRequestError;
use super::proposed::commit_lines;
use super::skeleton::shape;
use super::status::Git;

/// What the agent is asked to do when the repository — or the user — supplied a shape to fill.
const SKELETON_INSTRUCTIONS: &str = "\
Write the description for a pull request by filling in the template at the end of this message.

Keep the template's headings, their order, and any checklist it carries exactly as they are, and
replace only the parts meant to be filled in. Leave a section out only if it plainly does not apply.
Reply with the description and nothing else: no preamble, no explanation, no code fence.

";

/// The same where there is no shape to fill, so the answer's form is the agent's own.
const PLAIN_INSTRUCTIONS: &str = "\
Write the description for a pull request.

Say what it changes and why it changes it, as a few short paragraphs or a short list. Reply with the
description and nothing else: no preamble, no explanation, no code fence.

";

/// The line that introduces the branch's commits, and the one that introduces the shape to fill.
/// The template's is last in the prompt, so the final thing read is the form the answer must take.
const COMMITS_LABEL: &str = "Commits on this branch, newest first:\n";
const SKELETON_LABEL: &str = "Template to fill in:\n";

/// The most the fixed text around the parts can cost. The choice between the two forms is made
/// after the budget is set, so the budget has to hold for whichever is chosen.
const INSTRUCTIONS_HEADROOM: usize = longest(
    SKELETON_INSTRUCTIONS.len() + SKELETON_LABEL.len(),
    PLAIN_INSTRUCTIONS.len(),
) + COMMITS_LABEL.len();

const fn longest(one: usize, other: usize) -> usize {
    if one > other {
        one
    } else {
        other
    }
}

impl Git {
    /// The prompt that asks for a description of what `project`'s checked-out branch proposes
    /// against `base`, filling `skeleton` when there is one.
    ///
    /// Gated on the user having trusted the project, for the same reason a commit-message draft is:
    /// what runs is an agent CLI with the project as its working directory, and an agent CLI reads
    /// the project's own configuration.
    ///
    /// [`PullRequestError::NothingToDescribe`] when the branch holds nothing `base` does not, which
    /// is reached before a single agent is run.
    ///
    /// Reads the repository, so callers reach it through
    /// [`Facade::blocking`](crate::facade::Facade::blocking) rather than a runtime worker.
    pub fn pull_request_prompt(
        &self,
        project: ProjectId,
        root: &Path,
        base: &str,
        skeleton: &str,
    ) -> Result<String, PullRequestError> {
        if !self.trusted(project)? {
            return Err(PullRequestError::Untrusted);
        }
        let proposed = self.proposed(project, root, base)?;

        let context = format!("Merging branch {} into {base}.\n\n", proposed.head);
        let budget = ONE_SHOT_PROMPT_LIMIT
            .saturating_sub(INSTRUCTIONS_HEADROOM + context.len() + skeleton.len());
        let commits = commit_lines(&proposed.commits, budget);
        // The shape is last, so the final thing read is the form the answer must take — and
        // everything it is to be filled from has already been read by then.
        Ok(match shape(skeleton) {
            Some(skeleton) => format!(
                "{SKELETON_INSTRUCTIONS}{context}{COMMITS_LABEL}{commits}\n{SKELETON_LABEL}{skeleton}"
            ),
            None => format!("{PLAIN_INSTRUCTIONS}{context}{COMMITS_LABEL}{commits}"),
        })
    }
}

#[cfg(test)]
#[path = "description_tests.rs"]
mod tests;
