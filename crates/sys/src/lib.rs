//! OS monitoring adapters for Soloist (a driven adapter crate).
//!
//! Implements the core's monitoring ports against the real operating system, so the pure
//! core never reads the OS itself: the CPU/memory [`SysinfoMetricsProbe`] (over `sysinfo`)
//! and the [`ProcPortProbe`] (port discovery over `/proc`). The crate depends only on
//! `soloist-core` and its OS libraries — never the reverse (the dependency-direction guard
//! enforces it).

mod metrics;
mod portscan;

pub use metrics::SysinfoMetricsProbe;
pub use portscan::ProcPortProbe;
