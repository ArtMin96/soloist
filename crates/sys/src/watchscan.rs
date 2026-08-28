//! Enumerating a project's watch-worthy paths, honouring git's own ignore precedence: the OS
//! read behind the core's `WatchScanner`.
//!
//! `git status` never reads a gitignored file — by definition — so a watch registered on one can
//! never change the answer a status read produces. The watch set this scanner reports *is* the
//! set `git status` itself reads: `.gitignore`, `.git/info/exclude`, global excludes, and nested
//! ignore files, all applied the way `git` applies them. Walking is blocking filesystem I/O; the
//! core reaches this port off the runtime, the same way it already does for
//! [`FileWatcher::watch`](soloist_core::FileWatcher::watch).

use ignore::WalkBuilder;
use soloist_core::filewatch::{Scan, ScanRequest, ScannedPath, WatchScanner};
use soloist_core::vcs::STATE_DIR;

/// Scans a root for the paths worth watching, over the `ignore` crate's own gitignore-precedence
/// walker.
#[derive(Clone, Copy, Default)]
pub struct IgnoreWatchScanner;

impl IgnoreWatchScanner {
    pub fn new() -> Self {
        Self
    }
}

impl WatchScanner for IgnoreWatchScanner {
    fn scan(&self, request: ScanRequest) -> Scan {
        let ignored_names = request.ignored_names;
        let mut builder = WalkBuilder::new(&request.root);
        builder
            // The crate's own default (true) skips every dot-directory, but `git status` still
            // reads tracked ones like `.github` and `.vscode` — so hiding them here would shrink
            // the watch set below what git actually reads.
            .hidden(false)
            // `.ignore`/`.rgignore` are ripgrep's own filter files, not git's; honouring them
            // (the crate's default) would shrink the set below what `git status` reads.
            .ignore(false)
            .git_ignore(request.honour_repository_ignores)
            .git_global(request.honour_repository_ignores)
            .git_exclude(request.honour_repository_ignores)
            .parents(request.honour_repository_ignores)
            // A project need not be a repository at all — git-related rules then simply find
            // nothing to apply, rather than being skipped outright because no `.git` was found.
            .require_git(false)
            // A symlinked subtree must not be walked twice, or let the walk escape the project.
            .follow_links(false)
            .filter_entry(move |entry| {
                let Some(name) = entry.file_name().to_str() else {
                    return true;
                };
                name != STATE_DIR && !ignored_names.iter().any(|ignored| ignored == name)
            });

        let mut paths = Vec::new();
        let mut truncated = false;
        for entry in builder.build() {
            // An unreadable directory (permissions, a race with a deletion) is not a failure of
            // the scan as a whole — it is simply absent from the result, the same way a path
            // that never existed would be.
            let Ok(entry) = entry else {
                continue;
            };
            if paths.len() == request.ceiling {
                truncated = true;
                break;
            }
            let directory = entry.file_type().is_some_and(|kind| kind.is_dir());
            paths.push(ScannedPath {
                path: entry.into_path(),
                directory,
            });
        }

        Scan { paths, truncated }
    }
}
