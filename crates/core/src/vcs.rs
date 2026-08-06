//! Shared value types for version control: the closed [`ChangeKind`] and [`SyncState`]
//! discriminators, and the [`FileChange`]/[`BranchInfo`] vocabulary a working tree's state is
//! described in.
//!
//! These are value vocabulary — like [`process`](crate::process) — owned by no context. The git
//! context that reads them out of a repository, the event bus that announces a change, and the
//! surfaces that render them all depend on this module, so it must depend on nothing itself;
//! that is what keeps the graph acyclic.
//!
//! Only the types live here. Reading a repository, caching a status, and deciding when to
//! re-read it are the git context's ([`crate::git`]).

use serde::{Deserialize, Serialize};

/// What happened to one path, as version control classifies it. A closed set: every match over
/// it is exhaustive, so a new classification cannot be added without every reader answering
/// for it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// The file's contents differ.
    Modified,
    /// The file's type changed — a regular file became a symlink or a submodule, or back.
    TypeChanged,
    /// A path version control did not track before is now tracked.
    Added,
    /// A tracked path is gone.
    Deleted,
    /// The file moved, keeping enough of its contents to be recognised as the same file.
    Renamed,
    /// The file was copied from another tracked file.
    Copied,
    /// Version control does not track this path and has not been told to ignore it.
    Untracked,
    /// A merge left this path with content from more than one side, unresolved.
    Conflicted,
}

/// One path's change on each side of the index: what a commit would record (`staged`) and what
/// the working tree holds beyond it (`unstaged`). Both are `None` only for a path that is
/// listed for some other reason, so a reader never has to invent a meaning for "neither".
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct GitFileStatus {
    /// The change staged for the next commit, or `None` when the index matches the last commit.
    pub staged: Option<ChangeKind>,
    /// The change in the working tree beyond what is staged, or `None` when the working tree
    /// matches the index. An untracked path and an unresolved conflict are both reported here:
    /// neither is staged, and both are resolved by acting on the working tree.
    pub unstaged: Option<ChangeKind>,
}

/// One changed path in a working tree.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FileChange {
    /// The path relative to the repository root, separated by `/` as version control reports
    /// it — never an absolute path, so the same change reads the same wherever the repository
    /// is checked out.
    pub path: String,
    /// What changed, on each side of the index.
    pub status: GitFileStatus,
    /// Where a renamed or copied file came from, `None` otherwise.
    pub original_path: Option<String>,
}

/// One entry in a project's file listing.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    /// The path relative to the repository root, separated by `/` as version control reports
    /// it. A path ending in `/` is a whole directory rather than a file.
    pub path: String,
    /// Whether version control was told to ignore this path. An ignored *directory* is listed
    /// once, as itself, rather than walked — a build output folder is one entry, not the
    /// hundred thousand files inside it.
    pub ignored: bool,
}

/// How the checked-out branch stands against its upstream. A closed set carrying the counts
/// where it has them, so "how far ahead" is stated once rather than beside a separate flag
/// that could disagree with it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SyncState {
    /// There is no comparison to make: the branch tracks nothing, or it tracks something whose
    /// position is not known here because nothing has fetched it yet.
    /// [`BranchInfo::upstream`] says which.
    Unknown,
    /// The branch and its upstream are at the same commit.
    UpToDate,
    /// The branch holds commits the upstream does not.
    Ahead { ahead: u32 },
    /// The upstream holds commits the branch does not.
    Behind { behind: u32 },
    /// Each holds commits the other does not.
    Diverged { ahead: u32, behind: u32 },
}

impl SyncState {
    /// Classifies a branch from how many commits each side holds that the other does not — for
    /// a branch whose upstream position is known, which is the only case a pair of counts
    /// describes ([`SyncState::Unknown`] is the other).
    pub fn from_counts(ahead: u32, behind: u32) -> Self {
        match (ahead, behind) {
            (0, 0) => SyncState::UpToDate,
            (ahead, 0) => SyncState::Ahead { ahead },
            (0, behind) => SyncState::Behind { behind },
            (ahead, behind) => SyncState::Diverged { ahead, behind },
        }
    }
}

/// What is checked out and how it stands against its upstream.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct BranchInfo {
    /// The checked-out branch, or `None` when the head is detached at a commit.
    pub name: Option<String>,
    /// The upstream the branch tracks (a remote-qualified name), `None` when it tracks nothing.
    pub upstream: Option<String>,
    /// How the branch stands against that upstream.
    pub sync: SyncState,
}
