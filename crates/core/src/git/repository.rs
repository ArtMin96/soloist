//! The git context's own driven port: reading a repository, plus the no-op default.
//!
//! Defined here, in the git context, rather than in the shared port layer — a new repository
//! read is a change confined to this domain. The adapter (`crates/git`, over the system `git`
//! command-line tool) implements [`GitRepository`]; the core never runs a subprocess itself.
//!
//! The port speaks only in domain values ([`GitStatus`], [`crate::vcs`], [`GitError`]). Nothing
//! the tool printed crosses it — that anti-corruption seam is what lets the engine underneath
//! be replaced without a single core behaviour changing.

use std::path::Path;

use super::error::GitError;
use super::status::GitStatus;
use crate::vcs::ProjectFile;

/// Reads the state of a version-controlled working tree.
///
/// An implementation is **blocking**: it runs an external tool, so callers reach it from the
/// blocking pool ([`crate::facade::Facade::blocking`]) rather than a runtime worker. It must
/// return within a bounded time — a repository read that cannot finish is a
/// [`GitError::Timeout`], never a wait without end — and must leave no process behind.
pub trait GitRepository: Send + Sync {
    /// The working-tree status of the repository containing `root`: what is checked out, how it
    /// stands against its upstream, and every path that differs from the last commit.
    /// [`GitError::NotARepo`] for a path under no repository, which is an ordinary state rather
    /// than a fault.
    fn status(&self, root: &Path) -> Result<GitStatus, GitError>;

    /// Every path in the repository containing `root`: what it tracks, what it does not yet
    /// track, and what it was told to ignore. An ignored directory is one entry rather than
    /// its contents, which is what keeps the answer bounded for a working tree carrying build
    /// output. Same [`GitError::NotARepo`] meaning as [`GitRepository::status`].
    fn list_files(&self, root: &Path) -> Result<Vec<ProjectFile>, GitError>;
}

/// A [`GitRepository`] that reports every path as belonging to no repository — the default
/// until the real adapter is wired (headless tools, tests that do not exercise git).
///
/// It degrades **silently**, like every other optional driven port: the git context reads
/// [`GitError::NotARepo`] as "this project has no version control to show", so a core built
/// without the adapter behaves exactly as one opened on a folder that is not a repository.
#[derive(Clone, Copy, Default)]
pub struct NoopGitRepository;

impl GitRepository for NoopGitRepository {
    fn status(&self, _root: &Path) -> Result<GitStatus, GitError> {
        Err(GitError::NotARepo)
    }

    fn list_files(&self, _root: &Path) -> Result<Vec<ProjectFile>, GitError> {
        Err(GitError::NotARepo)
    }
}
