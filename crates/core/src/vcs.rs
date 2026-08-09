//! Shared value types for version control: the closed [`ChangeKind`] and [`SyncState`]
//! discriminators, and the [`FileChange`]/[`BranchInfo`]/[`Branch`] vocabulary a working tree's
//! state is described in.
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

/// Where one hunk of a unified diff falls on each side of the comparison, taken from its `@@`
/// line.
///
/// It is also how an action names one hunk. Within a path and a comparison the four numbers are
/// unique, and they change the moment the file does — so a stale request describes a hunk that
/// is no longer there and is refused, rather than being applied to whatever now occupies those
/// lines.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct HunkRange {
    /// The first line the hunk covers in the version being compared against.
    pub old_start: u32,
    /// How many lines it covers there — zero for a hunk that only adds.
    pub old_lines: u32,
    /// The first line the hunk covers in the version being compared.
    pub new_start: u32,
    /// How many lines it covers there — zero for a hunk that only removes.
    pub new_lines: u32,
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
    /// Where each hunk of `patch` falls, in the order it appears there — the vocabulary an
    /// action on one hunk is addressed in. A hunk left out of a truncated `patch` is left out
    /// of this too, so a reader can only name a hunk it was actually shown.
    pub hunks: Vec<HunkRange>,
    /// Whether the diff was longer than one read carries, leaving `patch` holding only its
    /// first hunks. Asking for the same diff in full carries the rest.
    pub truncated: bool,
}

/// The most of a commit's message body one entry carries. A message is prose somebody wrote, with no
/// ceiling of its own — a commit can hold a whole design document, and some do — while a page of fifty
/// of them crosses to a surface that renders subjects. Generous enough that no message anybody writes
/// by hand meets it, and past it the body is left out whole rather than cut, so what a reader is given
/// is either the message or plainly nothing at all.
pub const COMMIT_BODY_LIMIT: usize = 8 * 1024;

/// One commit, as a history read reports it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CommitEntry {
    /// The full object name, in hexadecimal, as version control reports it. Full rather than
    /// abbreviated, because an abbreviation is a rendering and can stop being unique.
    pub id: String,
    /// The first line of the message.
    pub subject: String,
    /// Everything the message says after its subject, as the author wrote it — empty for the many
    /// commits that say only their subject, which is why it is a string rather than an option, and
    /// empty as well for one longer than [`COMMIT_BODY_LIMIT`].
    pub body: String,
    /// Who wrote it, as the name version control recorded.
    pub author: String,
    /// When it was written, in seconds since the epoch. A number rather than a rendering, so a
    /// surface formats it in the reader's own locale and time zone.
    pub authored_at: i64,
    /// Whether it joins more than one line of history. A merge records no change anyone authored,
    /// so a reader — and anything learning from the log — treats it differently from a commit
    /// somebody wrote.
    pub merge: bool,
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

/// One branch a surface can offer to switch to.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Branch {
    /// The branch's own name, without the `refs/heads/` it is stored under.
    pub name: String,
    /// The upstream it tracks (a remote-qualified name), `None` when it tracks nothing.
    pub upstream: Option<String>,
    /// Whether this is the branch that is checked out — the one a switch has nothing to do for
    /// and a delete cannot touch.
    pub head: bool,
}

/// What a branch switcher can act on: the branches worth offering, and whether the working tree
/// has anything set aside — the other thing that surface offers, and the one fact that says
/// whether taking it back is an action at all.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Branches {
    /// The branches, most recently committed to first — which is both a bound on how many are
    /// carried and the order a switcher wants them in.
    pub entries: Vec<Branch>,
    /// Whether anything is set aside in the stash.
    pub stashed: bool,
}
