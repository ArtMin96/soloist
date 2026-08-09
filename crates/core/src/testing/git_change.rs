//! The change a working tree was asked to undergo, as the port received it.
//!
//! Its own module because it is the vocabulary a test writes its expectations in, rather than part
//! of the fake that records them — the same split [`git_fixtures`](super::git_fixtures) makes on
//! the reading side.

use crate::git::{BranchOp, Prompting, StashOp, SyncOp};
use crate::vcs::HunkRange;

/// One change a working tree was asked to undergo, as the port received it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GitChange {
    Stage {
        path: String,
        original_path: Option<String>,
    },
    Unstage {
        path: String,
        original_path: Option<String>,
    },
    Discard {
        path: String,
    },
    StageHunk {
        path: String,
        hunk: HunkRange,
    },
    UnstageHunk {
        path: String,
        hunk: HunkRange,
    },
    DiscardHunk {
        path: String,
        hunk: HunkRange,
    },
    Commit {
        message: String,
        amend: bool,
    },
    Branch {
        op: BranchOp,
        name: String,
    },
    Stash {
        op: StashOp,
    },
    Sync {
        op: SyncOp,
        prompting: Prompting,
    },
    AbortMerge,
}
