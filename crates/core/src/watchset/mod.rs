//! The single owner of Soloist's filesystem watch registrations.
//!
//! Three purposes need to know when a project's files change — the config reload, the
//! `restart_when_changed` policy, and the git rail — and until now each opened its own OS
//! watcher to find out (`crates/sys/src/filewatch.rs`'s ~4-instances-per-project ceiling this
//! module exists to close). [`ProjectWatchSet`] is the one owner instead: it holds the single
//! [`WatchSession`](crate::filewatch::WatchSession), plans what to watch within the app's
//! [`budget`], maintains it incrementally as directories appear and vanish, and serves every
//! consumer from one [`broadcast`](tokio::sync::broadcast) fan-out of changed paths. The three
//! reactors become plain consumers of that fan-out.
//!
//! A sibling of [`crate::filewatch`] rather than a member of it, deliberately: this module
//! names [`crate::projects`] and [`crate::supervisor`], both of which
//! [`crate::filewatch::ConfigWatchReactor`](crate::projects::ConfigWatchReactor) and
//! [`crate::filewatch::WatchReactor`] already name in the other direction, so putting it inside
//! `filewatch` would close a ring `scripts/check-core-cycles.sh` has no allow-list for.
//!
//! [`plan`] is the pure policy (given scan results, produce a registration plan and the
//! resulting [`WatchLimit`](crate::watch::WatchLimit)s); [`budget`] is the pure bookkeeping of
//! what the app may spend; [`set`] is the stateful loop that drives both over the real ports.

mod budget;
mod plan;
mod reconcile;
mod registry;
mod set;

pub use set::ProjectWatchSet;
