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

/// Which two versions of a path a diff compares. A closed set: every reader answers for each
/// case, so a comparison cannot be added without every surface saying what it shows for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffTarget {
    /// What the next commit would record: the last commit against the index.
    Staged,
    /// What the working tree holds beyond the index.
    Unstaged,
    /// The working tree against the last commit, whether staged or not.
    Head,
    /// The whole of a path version control does not track, which has no earlier version to be
    /// compared against. Resolved from a path's own status rather than asked for.
    Untracked,
}

/// One path's unified diff, as a reader is given it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FileDiff {
    /// The path relative to the repository root, as [`FileChange::path`] reports it.
    pub path: String,
    /// Where a renamed or copied file came from, `None` otherwise.
    pub original_path: Option<String>,
    /// Which comparison produced this: what was asked for, unless the path is untracked and
    /// there is only one comparison to make.
    pub target: DiffTarget,
    /// Whether version control reports the path as holding bytes rather than text. `patch` is
    /// then empty — there is nothing in it a reader could be shown but noise.
    pub binary: bool,
    /// The unified diff, its header included, or empty when the path does not differ at
    /// `target`. Always whole hunks, so it is a patch rather than a fragment of one.
    pub patch: String,
    /// Whether the diff was longer than one read carries, leaving `patch` holding only its
    /// first hunks. Asking for the same diff in full carries the rest.
    pub truncated: bool,
}

/// A path's contents, as a reader is given them.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FileContent {
    /// The file's text, or `None` when it holds bytes that are not text — which a reader is
    /// told about rather than shown.
    pub text: Option<String>,
    /// Whether the file was longer than one read carries, leaving `text` holding only its
    /// beginning.
    pub truncated: bool,
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
