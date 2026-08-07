//! Listing branches, moving between them, and the stash.
//!
//! The listing is asked for in a format of this adapter's own choosing, one field at a time with a
//! NUL between them, and read a record per line. A newline is safe as the record separator here and
//! nowhere else in this adapter: a ref name may not contain a control character at all, which
//! version control enforces when a name is created, so unlike a path a name cannot carry one.
//!
//! Every change here is handed straight to version control and reports what it said when it refuses
//! — a switch that would overwrite work, a branch holding commits nothing else holds, stashed
//! changes that no longer fit the working tree. Those accounts name the work in the way, which
//! nothing here could, and none of them is read: they are carried as the user's own to read.

use std::path::Path;

use soloist_core::{Branch, BranchOp, Branches, GitError, StashOp};

use crate::runner::{self, Run};

/// The fields each branch is reported in, in order: its own name, the upstream it tracks, and
/// whether it is the one checked out.
const BRANCH_FORMAT: &str = "--format=%(refname:short)%00%(upstream:short)%00%(HEAD)";

/// Where branches are kept. Naming it keeps tags, notes, and every remote's copy of a branch out of
/// a list whose every entry is meant to be something the working tree could be switched to.
const BRANCHES: &str = "refs/heads";

/// Where the stash is kept: one ref, whether it holds one set of changes or ten, so its presence is
/// the whole of what a surface needs to know before offering to take them back.
const STASH: &str = "refs/stash";

/// Most recently committed to first, which is the order a switcher wants and the one that decides
/// which branches a limit leaves out.
const RECENT_FIRST: &str = "--sort=-committerdate";

/// What the third field holds for the branch that is checked out.
const HEAD_MARKER: &str = "*";

/// How many fields each record holds.
const BRANCH_FIELDS: usize = 3;

/// The branches worth offering, and whether anything is stashed.
pub(crate) fn branches(root: &Path, limit: usize) -> Result<Branches, GitError> {
    let listed = runner::run(
        root,
        &[
            "for-each-ref",
            BRANCH_FORMAT,
            RECENT_FIRST,
            &format!("--count={limit}"),
            BRANCHES,
        ],
    )?;
    Ok(Branches {
        entries: parse(&listed),
        stashed: !runner::run(root, &["for-each-ref", "--count=1", STASH])?.is_empty(),
    })
}

/// Does `op` to the branch called `name`.
pub(crate) fn branch(root: &Path, op: BranchOp, name: &str) -> Result<(), GitError> {
    let args: &[&str] = match op {
        BranchOp::Create => &["switch", "--create"],
        BranchOp::Switch => &["switch"],
        // The unforced form, which refuses a branch holding commits no other branch holds. There
        // is no path in this adapter to the forced one.
        BranchOp::Delete => &["branch", "--delete"],
    };
    refusable(root, args, Some(name))
}

/// Moves what the working tree holds into the stash, or the most recent of it back out.
pub(crate) fn stash(root: &Path, op: StashOp) -> Result<(), GitError> {
    let args: &[&str] = match op {
        // Untracked files are deliberately left where they are: taking them would mean a file the
        // user made disappearing from the working tree, and nothing here does that.
        StashOp::Save => &["stash", "push"],
        StashOp::Pop => &["stash", "pop"],
    };
    refusable(root, args, None)
}

/// Runs one invocation whose refusal is worth carrying back to the person who asked for it.
fn refusable(root: &Path, command: &[&str], name: Option<&str>) -> Result<(), GitError> {
    let mut args = command.to_vec();
    args.extend(name);
    runner::run_with(
        root,
        &args,
        Run {
            report_refusal: true,
            ..Run::default()
        },
    )
    .map(|_| ())
}

/// Reads a branch listing. A record that does not hold every field is left out rather than guessed
/// at — the listing is a choice of surfaces to offer, so one entry nobody can act on is better left
/// unoffered than reported wrong.
fn parse(output: &[u8]) -> Vec<Branch> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|record| {
            let mut fields = record.splitn(BRANCH_FIELDS, '\0');
            let name = fields.next().filter(|name| !name.is_empty())?;
            let upstream = fields.next()?;
            Some(Branch {
                name: name.to_string(),
                upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
                head: fields.next()?.trim() == HEAD_MARKER,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "branch_tests.rs"]
mod tests;
