//! OS monitoring adapters for Soloist (a driven adapter crate).
//!
//! Implements the core's monitoring ports against the real operating system, so the pure
//! core never reads the OS itself. Today this is the CPU/memory [`SysinfoMetricsProbe`];
//! port discovery and readiness land here next. The crate depends only on `soloist-core`
//! and its OS libraries — never the reverse (the dependency-direction guard enforces it).

mod metrics;

pub use metrics::SysinfoMetricsProbe;
