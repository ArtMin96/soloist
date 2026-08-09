//! Parameter structs for the version-control tools.
//!
//! No tool here names a project: every one acts on the session's effective project, which the core
//! resolves from who the caller is. That is why there is no project argument to validate — there is
//! no way to address another project's repository from these tools at all.
//!
//! The two closed vocabularies a caller has to speak — which comparison a diff is, and how a pull
//! request is merged — are mirrored here as their own enums rather than reaching for the core's, so
//! the schema an agent reads offers exactly the choices this surface accepts.

use rmcp::schemars;
use serde::Deserialize;
use soloist_core::{DiffTarget, HunkRange, MergeMethod, NewPullRequest};

/// Which two versions of a path a diff compares.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiffTargetArg {
    /// What the next commit would record: the last commit against the index.
    Staged,
    /// What the working tree holds beyond the index.
    Unstaged,
    /// The working tree against the last commit, whether staged or not.
    Head,
}

impl From<DiffTargetArg> for DiffTarget {
    fn from(target: DiffTargetArg) -> Self {
        match target {
            DiffTargetArg::Staged => DiffTarget::Staged,
            DiffTargetArg::Unstaged => DiffTarget::Unstaged,
            DiffTargetArg::Head => DiffTarget::Head,
        }
    }
}

/// Arguments for reading one path's diff.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct GitDiffArg {
    /// The path relative to the repository root, as `git_status` reports it (`/` separated).
    pub(crate) path: String,
    /// Which comparison to read. A path version control does not track is read as the whole of
    /// itself whatever this asks for, and the answer says which comparison it actually is.
    pub(crate) target: DiffTargetArg,
    /// Read the whole diff however long it is. Omit for the capped read, which reports
    /// `truncated: true` when there was more — ask again with this set to get the rest.
    #[serde(default)]
    pub(crate) full: bool,
}

/// One hunk of a diff, named by where it falls on each side of the comparison — the four numbers
/// from its `@@` line, which `git_diff` reports for every hunk it returns.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct HunkArg {
    /// The first line the hunk covers in the version being compared against.
    pub(crate) old_start: u32,
    /// How many lines it covers there — zero for a hunk that only adds.
    pub(crate) old_lines: u32,
    /// The first line the hunk covers in the version being compared.
    pub(crate) new_start: u32,
    /// How many lines it covers there — zero for a hunk that only removes.
    pub(crate) new_lines: u32,
}

impl From<HunkArg> for HunkRange {
    fn from(hunk: HunkArg) -> Self {
        HunkRange {
            old_start: hunk.old_start,
            old_lines: hunk.old_lines,
            new_start: hunk.new_start,
            new_lines: hunk.new_lines,
        }
    }
}

/// Arguments for acting on one path, or on one hunk of it.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct GitPathArg {
    /// The path relative to the repository root, as `git_status` reports it (`/` separated).
    pub(crate) path: String,
    /// Act on only this hunk rather than the whole path. Take it from `git_diff`'s `hunks`; a hunk
    /// the file has moved past is refused rather than applied to whatever now occupies those lines.
    #[serde(default)]
    pub(crate) hunk: Option<HunkArg>,
}

/// Arguments for recording a commit.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct GitCommitArg {
    /// The commit message. The first line is the subject; a blank line then the body.
    pub(crate) message: String,
    /// Replace the last commit with this one instead of adding a commit. Rewrites what is
    /// committed, so it is refused on a project the user has not trusted like any other change.
    #[serde(default)]
    pub(crate) amend: bool,
}

/// Arguments naming one branch.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct GitBranchArg {
    /// The branch's own name, without `refs/heads/` — as `git_branches` reports it.
    pub(crate) name: String,
}

/// Arguments for proposing a pull request.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct GitCreatePullRequestArg {
    /// The title. Blank is refused.
    pub(crate) title: String,
    /// The description. Where `git_pull_request` offered a template, filling that shape in is what
    /// the repository expects.
    pub(crate) body: String,
    /// The branch to merge into — `git_pull_request` reports the one this repository defaults to.
    pub(crate) base: String,
    /// Open it as a draft, so review is not requested yet.
    #[serde(default)]
    pub(crate) draft: bool,
}

impl From<GitCreatePullRequestArg> for NewPullRequest {
    fn from(arg: GitCreatePullRequestArg) -> Self {
        NewPullRequest {
            title: arg.title,
            body: arg.body,
            base: arg.base,
            draft: arg.draft,
        }
    }
}

/// How a pull request's commits are put into its base branch.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MergeMethodArg {
    /// Keep every commit and record a merge.
    Merge,
    /// Fold them into one commit on the base.
    Squash,
    /// Replay them onto the base with no merge recorded.
    Rebase,
}

impl From<MergeMethodArg> for MergeMethod {
    fn from(method: MergeMethodArg) -> Self {
        match method {
            MergeMethodArg::Merge => MergeMethod::Merge,
            MergeMethodArg::Squash => MergeMethod::Squash,
            MergeMethodArg::Rebase => MergeMethod::Rebase,
        }
    }
}

/// Arguments for merging a pull request.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct GitMergePullRequestArg {
    /// The pull request's number, as `git_pull_request` or `git_pull_request_review` reports it.
    pub(crate) number: u64,
    /// How to merge. `git_pull_request` reports which methods this repository permits, its
    /// preferred one first; one it forbids is refused by the service.
    pub(crate) method: MergeMethodArg,
}
