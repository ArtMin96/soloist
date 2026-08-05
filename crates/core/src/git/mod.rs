//! The git bounded context (C9): what version control says about a project, and when to ask.
//!
//! Like the metrics and file-watch domains, this context owns *how version control works* here
//! — the port it depends on ([`GitRepository`], which it defines for itself), the failures it
//! speaks in ([`GitError`]), the read model it publishes ([`GitStatus`], over the shared
//! [`vcs`](crate::vcs) vocabulary), the per-project cache that serves it, and the
//! [`GitStatusWatchReactor`] that decides when a repository is worth re-reading, and the file
//! listing a browsing surface reads. Running the
//! tool is an adapter's job (`crates/git`, over the system `git` command line); a missing
//! adapter degrades to [`NoopGitRepository`], and a project simply shows no version control.
//!
//! Nothing the tool printed reaches here: the adapter parses its machine-readable output into
//! these types, so the core's behaviour cannot depend on the wording — or the language — of a
//! program it does not own.

mod error;
mod files;
mod repository;
mod status;
mod watch;

pub use error::GitError;
pub use repository::{GitRepository, NoopGitRepository};
pub use status::{Git, GitStatus};
pub use watch::GitStatusWatchReactor;
