//! The git bounded context (C9): what version control says about a project, when to ask, and
//! what a surface may change.
//!
//! Like the metrics and file-watch domains, this context owns *how version control works* here
//! — the port it depends on ([`GitRepository`], which it defines for itself), the failures it
//! speaks in ([`GitError`]), the read model it publishes ([`GitStatus`], over the shared
//! [`vcs`](crate::vcs) vocabulary), the per-project cache that serves it, and the
//! [`GitStatusWatchReactor`] that decides when a repository is worth re-reading, the file
//! listing a browsing surface reads, how much of one path's diff a reader is handed at once,
//! what an agent is told about a staged change when it is asked to describe it, what
//! exchanging commits with a remote is allowed to cost, and what proposing a branch's commits as
//! a pull request needs before it can be offered. Running the tools is an adapter's job
//! (`crates/git`, over the system `git` command line; `crates/forge`, over the `gh` one; the Tauri
//! shell, for handing a file to the desktop); a missing adapter degrades to
//! [`NoopGitRepository`], [`NoopGitForge`] or [`NoopFileOpener`], and a project simply shows no
//! version control, no pull requests, or no way to open a file elsewhere.
//!
//! Nothing the tool printed reaches here: the adapter parses its machine-readable output into
//! these types, so the core's behaviour cannot depend on the wording — or the language — of a
//! program it does not own.

mod branch;
mod commit;
mod description;
mod diff;
mod error;
mod exchange;
mod files;
mod forge;
mod handoff;
mod history;
mod message;
mod message_change;
mod opener;
mod path;
mod pr;
mod proposed;
mod repository;
mod review;
mod routing;
mod skeleton;
mod stage;
mod status;
mod suggestion;
mod sync;
mod watch;

pub use branch::BRANCH_PAGE_SIZE;
pub use diff::DiffExtent;
pub use error::{GitDraftError, GitError, GitWriteError};
pub use exchange::{Observer, Progress, Prompting, Stop, SyncOp};
pub use forge::{
    ForgeError, ForgeReadiness, ForgeRepository, GitForge, NewPullRequest, NoopGitForge,
    PullRequest, PullRequestState, PullRequestTemplate,
};
pub use handoff::{HandoffSubject, CHECK_LOG_LIMIT, HANDOFF_LIMIT};
pub use history::LOG_PAGE_SIZE;
pub use message::CommitIntent;
pub use opener::{FileOpener, NoopFileOpener, OpenError};
pub use pr::{PullRequestError, PullRequestSurface};
pub use repository::{
    BranchOp, Exchange, GitRepository, LogRange, NoopGitRepository, RawFileDiff, RawHunk, StashOp,
};
pub use review::{
    CheckRun, CheckState, MergeMethod, PullRequestReview, ReviewComment, ReviewLimits,
    ReviewThread, REVIEW_LIMITS,
};
pub use status::{Git, GitCapabilities, GitChangeCounts, GitLineCounts, GitStatus, GitStatusFacts};
pub use suggestion::PullRequestSuggestion;
pub use watch::GitStatusWatchReactor;
