//! The single owner of Soloist's filesystem watch registrations.
//!
//! Three purposes need to know when a project's files change — the config reload, the
//! `restart_when_changed` policy, and the git rail — and a separate OS watcher for each would
//! spend the system's watch limit several times over on the same directories.
//! [`ProjectWatchSet`] is their one owner instead: it holds the single
//! [`WatchSession`](crate::filewatch::WatchSession), plans what to watch within the app's
//! [`budget`], maintains it incrementally as directories appear and vanish, and serves every
//! consumer from one [`broadcast`](tokio::sync::broadcast) fan-out of changed paths. The three
//! reactors are plain consumers of that fan-out.
//!
//! A sibling of [`crate::filewatch`] rather than a member of it, deliberately: this module
//! names [`crate::projects`] and [`crate::supervisor`], both of which
//! [`crate::filewatch::ConfigWatchReactor`](crate::projects::ConfigWatchReactor) and
//! [`crate::filewatch::WatchReactor`] already name in the other direction, so putting it inside
//! `filewatch` would close a ring `scripts/check-core-cycles.sh` has no allow-list for.
//!
//! [`plan`] is the pure policy (given scan results, produce a registration plan and the
//! resulting [`WatchLimit`](crate::watch::WatchLimit)s); [`budget`] is the pure bookkeeping of
//! what the app may spend, wrapped with the refcounted held-path map in `registry`; [`set`]
//! holds the type vocabulary and the event loop, over `reconcile`'s planning half
//! (`resync`/`replan`) — split out for size, both `impl ProjectWatchSet`.

mod budget;
mod plan;
mod reconcile;
mod registry;
mod set;

pub use set::ProjectWatchSet;

use crate::filewatch::DEFAULT_IGNORES;

/// The directory names every scan this module asks for is told to skip, in the shape
/// [`ScanRequest::ignored_names`](crate::filewatch::ScanRequest::ignored_names) takes them.
fn ignored_names() -> Vec<String> {
    DEFAULT_IGNORES
        .iter()
        .map(|name| (*name).to_string())
        .collect()
}
