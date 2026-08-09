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
//! could. Only what the branch holds and its base does not is shown: a base merged in along the way
//! is not part of what is being proposed.
//!
//! **There is a ceiling.** The prompt is composed to [`ONE_SHOT_PROMPT_LIMIT`] rather than cut to
//! it. A template past [`SKELETON_LIMIT`] is dropped whole and a plain description asked for
//! instead, because half a skeleton would be filled in as if it were the whole of one.
//!
//! **The draft is advisory.** Nothing here or above it proposes anything: the text goes back to
//! whoever asked, to read and change before they ever press the button.

use std::path::Path;

use crate::agents::ONE_SHOT_PROMPT_LIMIT;
use crate::ids::ProjectId;
use crate::vcs::CommitEntry;

use super::branch::usable_branch_name;
use super::pr::PullRequestError;
use super::repository::LogRange;
use super::status::Git;

/// The longest description template that is worth handing over whole. Past it the shape has stopped
/// being a shape, and a filled-in half of one reads as a complete answer while missing most of what
/// the repository asked for.
const SKELETON_LIMIT: usize = 8 * 1024;

/// How many of the branch's commits are shown. Enough to describe any branch somebody opens a pull
/// request from by hand, and a ceiling on a branch that has been running for months.
const PROPOSED_COMMITS: usize = 50;

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
        if !usable_branch_name(base) {
            return Err(PullRequestError::UnusableBranchName);
        }
        let head = self
            .status(project, root)?
            .and_then(|status| status.branch.name)
            .ok_or(PullRequestError::DetachedHead)?;
        let proposed = self
            .history(project, root, LogRange::Since { base }, 0, PROPOSED_COMMITS)?
            .unwrap_or_default();
        if proposed.is_empty() {
            return Err(PullRequestError::NothingToDescribe);
        }

        let context = format!("Merging branch {head} into {base}.\n\n");
        let budget = ONE_SHOT_PROMPT_LIMIT
            .saturating_sub(INSTRUCTIONS_HEADROOM + context.len() + skeleton.len());
        let commits = list(&proposed, budget);
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

/// The skeleton to hand over, or `None` where there is none worth handing over: nothing was
/// supplied, or what was supplied is past the ceiling and would arrive as a fragment.
fn shape(skeleton: &str) -> Option<&str> {
    (!skeleton.trim().is_empty() && skeleton.len() <= SKELETON_LIMIT).then_some(skeleton)
}

/// One line per commit, newest first, within `budget` — dropping the oldest first, since the newest
/// are what the branch most recently became.
fn list(proposed: &[CommitEntry], budget: usize) -> String {
    let mut listed = String::new();
    for commit in proposed {
        let line = format!("- {}\n", commit.subject);
        if listed.len() + line.len() > budget {
            break;
        }
        listed.push_str(&line);
    }
    listed
}

#[cfg(test)]
#[path = "description_tests.rs"]
mod tests;
