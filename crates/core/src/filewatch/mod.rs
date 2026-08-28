//! File-watch restarts: restart a command when a watched file changes.
//!
//! Like the metrics and portscan domains, this context owns *how file-watching works* — the
//! OS read it depends on ([`FileWatcher`], the port it defines for itself) and the policy
//! that drives it ([`WatchReactor`] over the pure [`policy`]). The OS read is an adapter
//! (`crates/sys`, over `notify`); a missing adapter degrades to [`NoopFileWatcher`], and the
//! reactor simply never restarts. The supervisor exposes the watch-eligible commands and the
//! `file_restart` effect; this domain decides *when* a change warrants a restart.

mod policy;
mod reactor;
mod scan;
mod status;
mod watcher;

pub(crate) use policy::{compile, is_ignored, literal_prefix, DEFAULT_IGNORES};
pub use reactor::WatchReactor;
pub use scan::{NoopWatchScanner, Scan, ScanRequest, ScannedPath, WatchScanner};
pub(crate) use status::WatchStatus;
pub use watcher::{
    FileChange, FileChangeKind, FileWatcher, NoopFileWatcher, NoopWatchHandle, NoopWatchSession,
    WatchHandle, WatchSession,
};
