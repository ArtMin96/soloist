//! The file-watch domain's second driven port: which paths under a root are worth watching.
//!
//! [`FileWatcher`](super::FileWatcher) says how to watch a path once chosen; [`WatchScanner`]
//! says which paths that is. An implementation walks the filesystem honouring a repository's own
//! ignore rules where asked, and stops at a caller-imposed ceiling rather than materialising an
//! unbounded tree — the adapter (`crates/sys`, over the `ignore` crate) does the walking; the
//! core decides nothing about what a directory contains.

use std::path::PathBuf;

/// What to scan, and under what limits.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScanRequest {
    /// The directory to enumerate. Included in the result when it exists.
    pub root: PathBuf,
    /// Directory names never descended into, whatever the repository's own ignore rules say — an
    /// exclusion a project names explicitly rather than one git already applies.
    pub ignored_names: Vec<String>,
    /// Whether the repository's own ignore rules (`.gitignore`, `.git/info/exclude`, global
    /// excludes) apply. `false` for a directory a user's `restart_when_changed` glob names
    /// explicitly — a gitignored path a glob still names must still be watched.
    pub honour_repository_ignores: bool,
    /// The most paths the walk may report before it stops and says it was cut short.
    pub ceiling: usize,
}

/// One path a scan found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScannedPath {
    pub path: PathBuf,
    /// Whether it is a directory. A scan reports every directory it descends into, not only the
    /// leaves beneath it, so a caller can register each one without walking the tree itself.
    pub directory: bool,
}

/// What one scan found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scan {
    pub paths: Vec<ScannedPath>,
    /// Whether the walk stopped at [`ScanRequest::ceiling`] rather than exhausting the tree. A
    /// truncated scan under-reports what is there, so a caller must not treat it as complete.
    pub truncated: bool,
}

/// Enumerates the paths under a root that are worth watching.
///
/// Blocking: a real implementation walks the filesystem, so a caller reaches it off the runtime,
/// the same way it already does for [`FileWatcher::watch`](super::FileWatcher::watch).
pub trait WatchScanner: Send + Sync {
    fn scan(&self, request: ScanRequest) -> Scan;
}

/// A [`WatchScanner`] that reports the root unconditionally, without touching the filesystem —
/// core holds no OS-facing code, so it cannot check whether the root exists; that is the real
/// adapter's job. The default until the real adapter is wired. A build without it watches each
/// project's root and repository state and nothing deeper, which is the degraded mode this
/// subsystem already has a name for, not a failure.
#[derive(Clone, Copy, Default)]
pub struct NoopWatchScanner;

impl WatchScanner for NoopWatchScanner {
    fn scan(&self, request: ScanRequest) -> Scan {
        Scan {
            paths: vec![ScannedPath {
                path: request.root,
                directory: true,
            }],
            truncated: false,
        }
    }
}
