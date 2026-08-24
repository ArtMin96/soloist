//! Measuring added and deleted lines from machine-readable version-control output and visible
//! untracked text files.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use soloist_core::{ChangeKind, FileChange, GitError, GitLineCounts};

use super::{runner, BINARY_SNIFF, FILE_LIMIT, NOTHING};

/// The checked-out commit a normal working tree is compared with.
const HEAD: &str = "HEAD";

/// The machine question that distinguishes an unborn branch from a tracked diff that failed.
const VERIFY_HEAD_ARGS: &[&str] = &["rev-parse", "--verify", "--quiet", HEAD];

/// The exit status [`VERIFY_HEAD_ARGS`] uses when no commit is checked out.
const MISSING_HEAD_STATUS: i32 = 1;

/// The machine command that calculates this repository's empty-tree object name, including for
/// repositories using a non-default object format.
const EMPTY_TREE_ARGS: &[&str] = &["hash-object", "-t", "tree", NOTHING];

/// The most untracked content inspected across one status read. The per-file half reuses the
/// adapter's existing [`FILE_LIMIT`].
const UNTRACKED_TOTAL_LIMIT: usize = 8 * FILE_LIMIT;

/// Reads tracked totals from version control and adds bounded visible untracked text lines.
pub(super) fn read(root: &Path, changes: &[FileChange]) -> GitLineCounts {
    let mut counts = tracked(root);
    let (untracked, untracked_complete) = untracked_additions(root, changes);
    counts.additions = match counts.additions.checked_add(untracked) {
        Some(additions) => additions,
        None => {
            counts.complete = false;
            usize::MAX
        }
    };
    counts.complete &= untracked_complete;
    counts
}

fn tracked(root: &Path) -> GitLineCounts {
    match numstat(root, HEAD) {
        Ok(output) => parse(&output),
        Err(_) => match runner::run(root, VERIFY_HEAD_ARGS) {
            Err(GitError::Op {
                status: Some(MISSING_HEAD_STATUS),
            }) => unborn(root),
            Ok(_) | Err(_) => GitLineCounts::default(),
        },
    }
}

fn unborn(root: &Path) -> GitLineCounts {
    let Ok(output) = runner::run(root, EMPTY_TREE_ARGS) else {
        return GitLineCounts::default();
    };
    let Ok(object) = std::str::from_utf8(&output) else {
        return GitLineCounts::default();
    };
    let object = object.trim();
    if object.is_empty() {
        return GitLineCounts::default();
    }
    numstat(root, object)
        .map(|output| parse(&output))
        .unwrap_or_default()
}

fn numstat(root: &Path, base: &str) -> Result<Vec<u8>, GitError> {
    runner::run(
        root,
        &["diff", "--numstat", "-z", "--find-renames", base, "--"],
    )
}

fn parse(output: &[u8]) -> GitLineCounts {
    let mut counts = GitLineCounts {
        complete: true,
        ..GitLineCounts::default()
    };
    let mut records = output.split(|&byte| byte == 0);
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(3, |&byte| byte == b'\t');
        let (Some(additions), Some(deletions), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            counts.complete = false;
            continue;
        };
        if path.is_empty() && (records.next().is_none() || records.next().is_none()) {
            counts.complete = false;
        }
        if additions == b"-" && deletions == b"-" {
            continue;
        }
        let (Some(additions), Some(deletions)) = (number(additions), number(deletions)) else {
            counts.complete = false;
            continue;
        };
        counts.additions = match counts.additions.checked_add(additions) {
            Some(total) => total,
            None => {
                counts.complete = false;
                usize::MAX
            }
        };
        counts.deletions = match counts.deletions.checked_add(deletions) {
            Some(total) => total,
            None => {
                counts.complete = false;
                usize::MAX
            }
        };
    }
    counts
}

fn number(bytes: &[u8]) -> Option<usize> {
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

fn untracked_additions(root: &Path, changes: &[FileChange]) -> (usize, bool) {
    let mut budget = UNTRACKED_TOTAL_LIMIT;
    let mut additions = 0usize;
    let mut complete = true;
    for change in changes {
        if change.status.unstaged != Some(ChangeKind::Untracked) {
            continue;
        }
        if budget == 0 {
            complete = false;
            break;
        }
        let path = root.join(&change.path);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            complete = false;
            continue;
        };
        if !metadata.file_type().is_file() {
            continue;
        }
        if metadata.len() > FILE_LIMIT as u64 {
            complete = false;
            continue;
        }
        let allowance = budget.min(FILE_LIMIT);
        if metadata.len() > allowance as u64 {
            complete = false;
            continue;
        }
        let Ok(file) = File::open(path) else {
            complete = false;
            continue;
        };
        let mut bytes = Vec::new();
        let read = file.take(allowance as u64 + 1).read_to_end(&mut bytes);
        budget = budget.saturating_sub(bytes.len());
        if read.is_err() {
            complete = false;
            continue;
        }
        if bytes.len() > allowance {
            complete = false;
            continue;
        }
        if bytes.iter().take(BINARY_SNIFF).any(|&byte| byte == 0) {
            continue;
        }
        additions = additions.saturating_add(line_count(&bytes));
    }
    (additions, complete)
}

fn line_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&byte| byte == b'\n').count()
        + usize::from(!bytes.is_empty() && !bytes.ends_with(b"\n"))
}

#[cfg(test)]
#[path = "line_counts_tests.rs"]
mod tests;
