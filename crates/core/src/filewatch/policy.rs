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
/// Solo documents no ignore list, so this default set is ours: the build, VCS, and dependency
/// trees that churn constantly (a `cargo build` rewrites `target/`, `npm install` rewrites
/// `node_modules/`) and would otherwise cause restart storms.
pub(crate) const DEFAULT_IGNORES: [&str; 5] = [".git", "node_modules", "target", "dist", ".venv"];

/// The directory a glob is anchored at: its leading components up to the first that carries a
/// glob metacharacter (`*`, `?`, `[`, `{`), excluding the pattern's final component (the file
/// position, never a directory to watch). `None` for a pattern with nothing before that —
/// anchored at the root itself, or its very first component is already a metacharacter.
///
/// Shared with [`crate::watchset`], which scans this directory with the repository's own ignore
/// rules disabled: a glob names it explicitly, so a gitignored prefix (`dist/config.json`) must
/// still be watched even though the whole-tree scan would skip `dist`.
pub(crate) fn literal_prefix(pattern: &str) -> Option<PathBuf> {
    let components: Vec<&str> = pattern.split('/').collect();
    let mut literal = Vec::new();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        if component.contains(['*', '?', '[', '{']) {
            break;
        }
        literal.push(*component);
    }
    if literal.is_empty() {
        None
    } else {
        Some(literal.into_iter().collect())
    }
}

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

/// Whether `relative` — a path relative to a project root — lies inside a default-ignored
/// directory at any depth. Shared with the git context's watch, so the trees whose churn is
/// never worth reacting to are named in exactly one place.
pub(crate) fn is_ignored(relative: &Path) -> bool {
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
