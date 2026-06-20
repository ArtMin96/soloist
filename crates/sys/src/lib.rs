//! OS adapters for Soloist (a driven adapter crate).
//!
//! Implements the core's driven ports against the real operating system, so the pure core
//! never reads the OS itself: the CPU/memory [`SysinfoMetricsProbe`] (over `sysinfo`), the
//! [`ProcPortProbe`] (port discovery over `/proc`), and the [`NotifyFileWatcher`] (recursive
//! file watching over `notify`). The crate depends only on `soloist-core` and its OS
//! libraries — never the reverse (the dependency-direction guard enforces it).

mod filewatch;
mod metrics;
mod portscan;

pub use filewatch::NotifyFileWatcher;
pub use metrics::SysinfoMetricsProbe;
pub use portscan::ProcPortProbe;
