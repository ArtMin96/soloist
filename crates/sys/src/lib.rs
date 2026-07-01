//! OS adapters for Soloist (a driven adapter crate).
//!
//! Implements the core's driven ports against the real operating system, so the pure core
//! never reads the OS itself: the CPU/memory [`ProcMetricsProbe`] and the [`ProcPortProbe`]
//! (both over `/proc`), the [`NotifyFileWatcher`] (recursive file watching over `notify`),
//! the [`CommandVersionProbe`] (agent-CLI auto-detection via `--version`), the
//! [`CommandShellEnvProbe`] (login-shell environment capture via `$SHELL -ilc env`), and the
//! [`CommandSummaryRunner`] (headless auto-summarization via `$SHELL -lc`). The crate depends
//! only on `soloist-core` and its OS libraries — never the reverse (the dependency-direction
//! guard enforces it).

mod agents;
mod filewatch;
mod metrics;
mod portscan;
mod proc;
mod shell;
mod shellenv;
mod summarize;

pub use agents::CommandVersionProbe;
pub use filewatch::NotifyFileWatcher;
pub use metrics::ProcMetricsProbe;
pub use portscan::ProcPortProbe;
pub use shellenv::CommandShellEnvProbe;
pub use summarize::CommandSummaryRunner;
