//! What an agent is told about a staged change when it is asked to draft a commit message.
//!
//! Four rules decide what it sees, and all four exist because the answer is only as good as the
//! subject.
//!
//! **Not everything staged says anything.** A resolver's own record of what it picked, and a file a
//! tool wrote rather than a person, describe no intent — they are noise that costs prompt and
//! misleads a reader of it. They are left out, and a change made only of them has nothing to
//! describe at all, which is refused before a single subprocess is spent.
//!
//! **A diff says what changed and never why.** So the change arrives with what little context is
//! cheap and honest: the branch it is on, and the subjects of the repository's own recent commits —
//! which is the only place the house's way of writing one is written down. They are examples of
//! *form*, stated as such, never material to copy.
//!
//! **There is a ceiling, and it falls on a whole path.** A prompt is composed to fit
//! [`ONE_SHOT_PROMPT_LIMIT`] rather than cut to it, and the parts are budgeted in the order they
//! matter: what is being asked, then the context, then as much of the change as is left. Past the
//! ceiling every path is described by name and by what happened to it instead, because a summary of
//! the whole change is more useful than a complete diff of the first tenth of it.
//!
//! **The draft is advisory.** Nothing here or above it commits: the text goes back to whoever asked,
//! to read and change first. That is why the composition can afford to leave things out.

use std::path::Path;

use crate::agents::ONE_SHOT_PROMPT_LIMIT;
use crate::ids::ProjectId;
use crate::sync::lock;
use crate::vcs::{BranchInfo, ChangeKind, CommitEntry, DiffTarget, FileChange};

use super::error::{GitDraftError, GitError};
use super::repository::LogRange;
use super::status::Git;

/// File names whose contents describe nothing about a change: a dependency resolver's own record of
/// what it picked. Each is written by a tool, is often thousands of lines, and says only that
/// something else changed — which the change that caused it already says better.
const RESOLVED_NAMES: [&str; 6] = [
    "package-lock.json",
    "packages.lock.json",
    "pnpm-lock.yaml",
    "bun.lockb",
    "go.sum",
    "Package.resolved",
];

/// Suffixes of paths a tool wrote rather than a person. `.lock`/`.lockfile` covers the resolvers
/// that name their record after the manifest (`Cargo.lock`, `yarn.lock`, `Gemfile.lock`,
/// `poetry.lock`, `flake.lock`, `gradle.lockfile`); the rest are build output that mirrors a source
/// change already in the same commit.
const GENERATED_SUFFIXES: [&str; 5] = [".lock", ".lockfile", ".min.js", ".min.css", ".map"];

/// How many staged paths are described by their diff before the whole change is summarised instead.
///
/// Each one costs a read of the repository, so this bounds the work as well as the prompt: a change
/// touching hundreds of files is summarised rather than read hundreds of times, which is also the
/// better description of it.
const DESCRIBED_PATH_LIMIT: usize = 48;

/// How many recent commit subjects are shown as examples of how this repository writes one.
const VOICE_EXAMPLES: usize = 10;

/// How many recent commits are looked at to find that many. More than [`VOICE_EXAMPLES`], because
/// the ones nobody authored are passed over and a run of merges would otherwise leave none.
const VOICE_EXAMPLE_SCAN: usize = 30;

/// Room kept back from the path summary so the line saying how many were left out always fits.
const REMAINDER_HEADROOM: usize = 64;

/// The prefix version control gives a revert's subject, which is another commit's subject quoted.
/// Not anybody's writing, so it teaches nothing about the house voice.
const REVERT_PREFIX: &str = "Revert \"";

/// What marks an author as a program rather than a person, by the forge convention that appends it
/// to a machine account's name. A commit nobody wrote is not an example of how anybody writes.
const BOT_SUFFIX: &str = "[bot]";

/// What the agent is asked to do, ahead of a change described by its diffs.
const PATCH_INSTRUCTIONS: &str = "\
Write a git commit message for the staged change below.

Reply with the message and nothing else: no preamble, no explanation, no code fence. Use a short
imperative subject line of at most 72 characters. Add a body only if the change needs one, separated
from the subject by a blank line, wrapped at 72 characters, saying why rather than restating the
diff.

";

/// The same, ahead of a change too large to show, described by its paths.
const SUMMARY_INSTRUCTIONS: &str = "\
Write a git commit message for the staged change below.

The change is too large to include, so only the files it touches are listed. Describe the change at
that level rather than inventing detail about the contents.

Reply with the message and nothing else: no preamble, no explanation, no code fence. Use a short
imperative subject line of at most 72 characters. Add a body only if the change needs one, separated
from the subject by a blank line, wrapped at 72 characters.

";

/// The line that introduces the change itself, in each of the two forms it can take. It is the last
/// thing before the subject, so nothing can be read as part of the change that is not the change.
const PATCH_LABEL: &str = "Staged change:\n";
const PATHS_LABEL: &str = "Staged files:\n";

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

impl Git {
    /// The prompt that asks for a commit message describing what is staged in `project`.
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
        let voice = self.voice(project, root)?;
        let budget = ONE_SHOT_PROMPT_LIMIT
            .saturating_sub(INSTRUCTIONS_HEADROOM + context.len() + voice.len());
        // What is being asked, then the context it is being asked in, then the change — so nothing
        // ahead of the label can be mistaken for part of the change, and nothing after it for
        // instructions.
        Ok(match self.describe(project, root, &staged, budget)? {
            Description::Patches(patches) => {
                format!("{PATCH_INSTRUCTIONS}{context}{voice}{PATCH_LABEL}{patches}")
            }
            Description::Paths(paths) => {
                format!("{SUMMARY_INSTRUCTIONS}{context}{voice}{PATHS_LABEL}{paths}")
            }
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

    /// How the staged change is described: by its patches while they fit `budget`, otherwise by its
    /// paths.
    ///
    /// Reading stops as soon as the patches pass the budget, so the fallback costs at most the reads
    /// already made — and for a change over [`DESCRIBED_PATH_LIMIT`] paths, none at all.
    fn describe(
        &self,
        project: ProjectId,
        root: &Path,
        staged: &[&FileChange],
        budget: usize,
    ) -> Result<Description, GitError> {
        if staged.len() > DESCRIBED_PATH_LIMIT {
            return Ok(Description::Paths(summarise(staged, budget)));
        }
        let mut patches = String::new();
        for change in staged {
            let Some(patch) = self.staged_patch(project, root, change)? else {
                continue;
            };
            if patches.len() + patch.len() > budget {
                return Ok(Description::Paths(summarise(staged, budget)));
            }
            patches.push_str(&patch);
        }
        // Every staged path was either binary or no longer differs from the last commit by the time
        // it was read, so there is no patch to show — the paths themselves are all that is left to
        // describe, and they still describe something.
        if patches.is_empty() {
            return Ok(Description::Paths(summarise(staged, budget)));
        }
        Ok(Description::Patches(patches))
    }

    /// One staged path's patch, or `None` when there is nothing in it to read: a path holding bytes
    /// rather than text, or one whose staged side turned out not to differ.
    fn staged_patch(
        &self,
        project: ProjectId,
        root: &Path,
        change: &FileChange,
    ) -> Result<Option<String>, GitError> {
        let gate = self.gate(project);
        let _running = lock(&gate);
        let raw = match self.repository.diff(
            root,
            DiffTarget::Staged,
            &change.path,
            change.original_path.as_deref(),
        ) {
            Ok(raw) => raw,
            Err(GitError::NotARepo) => return Ok(None),
            Err(err) => return Err(err),
        };
        if raw.binary || raw.hunks.is_empty() {
            return Ok(None);
        }
        let mut patch = raw.header;
        for hunk in &raw.hunks {
            patch.push_str(&hunk.text);
        }
        Ok(Some(patch))
    }
}

/// The form the staged change is described in.
enum Description {
    /// The patches themselves, joined as version control produced them.
    Patches(String),
    /// One line per path: what happened to it, and where.
    Paths(String),
}

/// Where the change is being made, in one line. Cheap — the status already carries it — and worth
/// saying, because a branch is often the only place its purpose is named.
fn describe_branch(branch: &BranchInfo) -> String {
    match &branch.name {
        Some(name) => format!("On branch {name}.\n\n"),
        None => String::new(),
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

/// One line per staged path — what happened, then the path, and where it came from when it moved —
/// within `budget`, saying how many were left out when it runs out of room.
fn summarise(staged: &[&FileChange], budget: usize) -> String {
    let mut summary = String::new();
    let mut named = 0;
    for change in staged {
        let line = summary_line(change);
        if summary.len() + line.len() + REMAINDER_HEADROOM > budget {
            break;
        }
        summary.push_str(&line);
        named += 1;
    }
    let left_out = staged.len() - named;
    if left_out > 0 {
        summary.push_str(&format!("and {left_out} more files\n"));
    }
    summary
}

/// One path's line of the summary.
fn summary_line(change: &FileChange) -> String {
    let happened = change.status.staged.map_or(CHANGED, described);
    match &change.original_path {
        Some(from) => format!("{happened} {} (from {from})\n", change.path),
        None => format!("{happened} {}\n", change.path),
    }
}

/// What a staged path is said to have had happen to it. Words rather than version control's letters,
/// because this is read as prose by something that answers in prose.
fn described(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Modified => "modified",
        ChangeKind::TypeChanged => "changed type of",
        ChangeKind::Added => "added",
        ChangeKind::Deleted => "deleted",
        ChangeKind::Renamed => "renamed",
        ChangeKind::Copied => "copied",
        ChangeKind::Untracked | ChangeKind::Conflicted => CHANGED,
    }
}

/// What a path is said to have had happen to it when nothing more precise applies — an unresolved
/// merge, or an untracked path, neither of which is a staged classification version control reports.
const CHANGED: &str = "changed";

/// Whether a path's contents say anything about the intent of the change that touched it.
fn describes_intent(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    !RESOLVED_NAMES.contains(&name)
        && !GENERATED_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
