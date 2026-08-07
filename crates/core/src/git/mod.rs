//! The git bounded context (C9): what version control says about a project, when to ask, and
//! what a surface may change.
//!
//! Like the metrics and file-watch domains, this context owns *how version control works* here
//! — the port it depends on ([`GitRepository`], which it defines for itself), the failures it
//! speaks in ([`GitError`]), the read model it publishes ([`GitStatus`], over the shared
//! [`vcs`](crate::vcs) vocabulary), the per-project cache that serves it, and the
//! [`GitStatusWatchReactor`] that decides when a repository is worth re-reading, the file
//! listing a browsing surface reads, how much of one path's diff a reader is handed at once, and
//! what an agent is told about a staged change when it is asked to describe it. Running the
//! tool is an adapter's job (`crates/git`, over the system `git` command line); a missing
//! adapter degrades to [`NoopGitRepository`], and a project simply shows no version control.
//!
//! Nothing the tool printed reaches here: the adapter parses its machine-readable output into
//! these types, so the core's behaviour cannot depend on the wording — or the language — of a
//! program it does not own.

mod commit;
mod diff;
mod error;
mod files;
mod history;
mod message;
mod path;
mod repository;
mod stage;
mod status;
mod watch;

pub use diff::DiffExtent;
pub use error::{GitDraftError, GitError, GitWriteError};
pub use history::LOG_PAGE_SIZE;
pub use repository::{GitRepository, NoopGitRepository, RawFileDiff, RawHunk};
pub use status::{Git, GitStatus};
pub use watch::GitStatusWatchReactor;
