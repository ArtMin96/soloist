//! OS adapters for Soloist (a driven adapter crate).
//!
//! Implements the core's driven ports against the real operating system, so the pure core
//! never reads the OS itself: the CPU/memory [`ProcMetricsProbe`] and the [`ProcPortProbe`]
//! (both over `/proc`), the [`NotifyFileWatcher`] (recursive file watching over `notify`),
//! the [`IgnoreWatchScanner`] (gitignore-aware watch-set enumeration over `ignore`), the
//! [`CommandVersionProbe`] (agent-CLI auto-detection via `--version`), and the
//! [`CommandShellEnvProbe`] (login-shell environment capture via `$SHELL -ilc env`). The
//! crate depends only on `soloist-core` and its OS libraries — never the reverse (the
//! dependency-direction guard enforces it).

mod agents;
mod filewatch;
mod metrics;
mod portscan;
mod proc;
mod shellenv;
mod watchscan;

pub use agents::CommandVersionProbe;
pub use filewatch::NotifyFileWatcher;
pub use metrics::ProcMetricsProbe;
pub use portscan::ProcPortProbe;
pub use shellenv::CommandShellEnvProbe;
pub use watchscan::IgnoreWatchScanner;
