//! The pure file-watch matching policy: does a changed path restart a given command?
//!
//! A command watches a set of globs (`restart_when_changed`) interpreted **relative to its
//! project root**, with `*` matching across path separators (Solo's documented behavior).
//! Changes inside a default-ignored directory never count. This module is pure — no clock,
//! no I/O — so it is exhaustively unit-testable on its own.

use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::ids::ProcessId;

/// Directories whose contents never trigger a file-watch restart, regardless of the globs.
/// Solo's ignore list is undocumented (`plan/05` §4); this is our decision, recorded in
/// `KNOWN-DIVERGENCES.md`. These are the build/VCS/dependency trees that change constantly
/// and would otherwise cause restart storms.
pub(crate) const DEFAULT_IGNORES: [&str; 5] = [".git", "node_modules", "target", "dist", ".venv"];

/// Compiles a command's `restart_when_changed` globs into a matcher, or `None` when the list
/// is empty or every pattern is invalid — in which case the command is not watched. Invalid
/// patterns are skipped so one typo does not silently disable the rest. `*` is left to match
/// across path separators (globset's default), matching Solo's documented glob semantics.
pub(crate) fn compile(globs: &[String]) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut any = false;
    for pattern in globs {
        if let Ok(glob) = Glob::new(pattern) {
            builder.add(glob);
            any = true;
        }
    }
    if !any {
        return None;
    }
    builder.build().ok()
}

/// Whether `relative` lies inside a default-ignored directory at any depth.
fn is_ignored(relative: &Path) -> bool {
    relative.components().any(|component| {
        matches!(component, Component::Normal(name)
            if DEFAULT_IGNORES.iter().any(|ignored| name == OsStr::new(ignored)))
    })
}

/// A compiled watch rule for one command: the project root its globs are relative to and the
/// matcher built from them.
pub(crate) struct WatchRule {
    pub(crate) id: ProcessId,
    root: PathBuf,
    set: GlobSet,
}

impl WatchRule {
    pub(crate) fn new(id: ProcessId, root: PathBuf, set: GlobSet) -> Self {
        Self { id, root, set }
    }

    /// Whether a change to `changed` (an absolute path) should restart this rule's command:
    /// the path lies under the project root, not inside a default-ignored directory, and
    /// matches one of the command's globs (evaluated relative to the root).
    pub(crate) fn matches(&self, changed: &Path) -> bool {
        let Ok(relative) = changed.strip_prefix(&self.root) else {
            return false;
        };
        !is_ignored(relative) && self.set.is_match(relative)
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
