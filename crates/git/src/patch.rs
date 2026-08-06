//! Building the smallest patch that carries out one hunk's worth of a change.
//!
//! Staging, unstaging and discarding a single hunk are all the same move: take the diff version
//! control just produced, keep exactly one hunk of it with the header that hunk has to follow,
//! and hand that back to `git apply` — forwards to record it, reversed to take it away. It is
//! the approach the terminal clients settled on, and its virtue is that nothing here has to
//! *understand* the change: the hunk is version control's own bytes, unedited, so there is no
//! way for a rebuilt context line to differ from the file it has to match.
//!
//! Two rules keep that true. The hunk is found by the range it states rather than by its place
//! in a list, so a request built against a diff the file has since moved past finds nothing and
//! changes nothing. And a hunk that arrived carrying a path's own header keeps it and no other,
//! so the patch never names two files.

use soloist_core::{HunkRange, RawFileDiff};

/// What begins one path's block of a unified diff.
const FILE: &str = "diff --git ";

/// The one hunk of `diff` that falls at `hunk`, as a patch of its own — or `None` when the diff
/// no longer holds a hunk there.
pub(crate) fn one_hunk(diff: &RawFileDiff, hunk: HunkRange) -> Option<String> {
    let found = diff
        .hunks
        .iter()
        .find(|candidate| candidate.range == hunk)?;
    if found.text.starts_with(FILE) {
        return Some(found.text.clone());
    }
    Some(format!("{}{}", diff.header, found.text))
}

#[cfg(test)]
#[path = "patch_tests.rs"]
mod tests;
