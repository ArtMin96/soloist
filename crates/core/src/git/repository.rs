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
use crate::vcs::{DiffTarget, FileContent, HunkRange, ProjectFile};

/// One hunk of a path's diff: where it falls, and the text that says how.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RawHunk {
    /// Where the hunk falls on each side, read from its `@@` line.
    pub range: HunkRange,
    /// The hunk itself, its own `@@` line included.
    pub text: String,
}

/// One path's unified diff as version control produced it, split where its hunks begin.
///
/// The split is the adapter's, because knowing where a hunk starts is knowing the patch format
/// — and it is what lets the context hand a reader the first hunks of a very long diff without
/// handing them a patch that stops halfway through one.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RawFileDiff {
    /// Whether version control reports the path as holding bytes rather than text.
    pub binary: bool,
    /// Everything before the first hunk — what any hunk of this patch has to be preceded by.
    pub header: String,
    /// Each hunk in turn.
    pub hunks: Vec<RawHunk>,
}

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

    /// How `path` differs at `target`, split where its hunks begin. `original_path` is where a
    /// renamed or copied path came from, which version control has to be told: a rename is
    /// recognised only when both of its names are asked about together.
    ///
    /// A path that does not differ is an empty diff rather than a failure — the ordinary answer
    /// for asking about the staged side of a change that is only in the working tree.
    fn diff(
        &self,
        root: &Path,
        target: DiffTarget,
        path: &str,
        original_path: Option<&str>,
    ) -> Result<RawFileDiff, GitError>;

    /// The working tree's copy of `path`, bounded: past the adapter's ceiling only the
    /// beginning arrives, marked as [`FileContent::truncated`]. `Ok(None)` for a path that is
    /// not there, which is ordinary for a listing read a moment before the file was removed.
    fn read_file(&self, root: &Path, path: &str) -> Result<Option<FileContent>, GitError>;

    /// Records everything `path` currently holds in the index, so the next commit would carry
    /// it. `original_path` is where a renamed path came from, which has to be named too or its
    /// disappearance is left unrecorded.
    fn stage(&self, root: &Path, path: &str, original_path: Option<&str>) -> Result<(), GitError>;

    /// Takes `path` back out of the index, leaving the working tree alone: after it the index
    /// holds what the last commit does, and every difference is unstaged again.
    fn unstage(&self, root: &Path, path: &str, original_path: Option<&str>)
        -> Result<(), GitError>;

    /// Throws away what the working tree holds beyond the index for `path`, restoring it from
    /// the index. It cannot reach past the index, so nothing already staged and nothing already
    /// committed can be lost through it.
    fn discard(&self, root: &Path, path: &str) -> Result<(), GitError>;

    /// Records only `hunk` of `path`'s unstaged change in the index, leaving the rest of the
    /// working tree's changes where they are.
    ///
    /// [`GitError::HunkGone`] when the unstaged diff no longer holds a hunk at that range —
    /// the file moved on between being read and being acted on, and applying the request
    /// anywhere else would change lines the caller never saw.
    fn stage_hunk(
        &self,
        root: &Path,
        path: &str,
        original_path: Option<&str>,
        hunk: HunkRange,
    ) -> Result<(), GitError>;

    /// Takes only `hunk` of `path`'s staged change back out of the index. Same
    /// [`GitError::HunkGone`] meaning as [`GitRepository::stage_hunk`], against the staged diff.
    fn unstage_hunk(
        &self,
        root: &Path,
        path: &str,
        original_path: Option<&str>,
        hunk: HunkRange,
    ) -> Result<(), GitError>;

    /// Throws away only `hunk` of `path`'s unstaged change, restoring those lines from the
    /// index. Same [`GitError::HunkGone`] meaning as [`GitRepository::stage_hunk`].
    fn discard_hunk(&self, root: &Path, path: &str, hunk: HunkRange) -> Result<(), GitError>;

    /// Records the index as a commit carrying `message`, or replaces the last commit with it
    /// when `amend` — which rewrites what is committed and never touches the working tree.
    ///
    /// The user's own hooks, signing key, author and configuration apply, because this is their
    /// `git` running: that is the whole reason the engine is the command line. A hook that
    /// refuses is a [`GitError::Refused`] carrying what it wrote, which is the one diagnostic
    /// worth showing — it is the user's own hook talking, not the tool.
    fn commit(&self, root: &Path, message: &str, amend: bool) -> Result<(), GitError>;
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

    fn diff(
        &self,
        _root: &Path,
        _target: DiffTarget,
        _path: &str,
        _original_path: Option<&str>,
    ) -> Result<RawFileDiff, GitError> {
        Err(GitError::NotARepo)
    }

    fn read_file(&self, _root: &Path, _path: &str) -> Result<Option<FileContent>, GitError> {
        Err(GitError::NotARepo)
    }

    fn stage(
        &self,
        _root: &Path,
        _path: &str,
        _original_path: Option<&str>,
    ) -> Result<(), GitError> {
        Err(GitError::NotARepo)
    }

    fn unstage(
        &self,
        _root: &Path,
        _path: &str,
        _original_path: Option<&str>,
    ) -> Result<(), GitError> {
        Err(GitError::NotARepo)
    }

    fn discard(&self, _root: &Path, _path: &str) -> Result<(), GitError> {
        Err(GitError::NotARepo)
    }

    fn stage_hunk(
        &self,
        _root: &Path,
        _path: &str,
        _original_path: Option<&str>,
        _hunk: HunkRange,
    ) -> Result<(), GitError> {
        Err(GitError::NotARepo)
    }

    fn unstage_hunk(
        &self,
        _root: &Path,
        _path: &str,
        _original_path: Option<&str>,
        _hunk: HunkRange,
    ) -> Result<(), GitError> {
        Err(GitError::NotARepo)
    }

    fn discard_hunk(&self, _root: &Path, _path: &str, _hunk: HunkRange) -> Result<(), GitError> {
        Err(GitError::NotARepo)
    }

    fn commit(&self, _root: &Path, _message: &str, _amend: bool) -> Result<(), GitError> {
        Err(GitError::NotARepo)
    }
}
