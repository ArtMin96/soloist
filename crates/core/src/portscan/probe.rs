//! The port-discovery domain's own port: reading which TCP ports process groups are
//! listening on. The adapter (`crates/sys`) reads `/proc`; the core never touches it.

use std::collections::HashMap;

/// Discovers the TCP ports managed process groups are listening on.
///
/// Batched like [`crate::metrics::MetricsProbe`]: an implementation reads its OS view once
/// per call across every requested group, so a tick costs one `/proc` sweep rather than one
/// per group. A group is identified by its leader `pgid`; the result aggregates every
/// process **in that process group** (matched by group id, so a reparented descendant is
/// still counted — unlike the metrics probe's subtree approximation).
pub trait PortProbe: Send + Sync {
    /// The distinct LISTEN-state TCP ports each requested group (by leader `pgid`) currently
    /// has bound, sorted ascending, keyed by `pgid`. Best-effort: an unreadable `/proc`
    /// entry yields fewer ports rather than an error, and a group with none maps to an empty
    /// list (kept, not omitted, so the scanner can clear a port that has gone away) —
    /// discovery never fails the core.
    fn listening_ports(&self, groups: &[i32]) -> HashMap<i32, Vec<u16>>;
}

/// A [`PortProbe`] that finds nothing — the default until the OS adapter is wired (headless
/// tools, tests that do not exercise discovery). The scanner then reports no ports.
#[derive(Clone, Copy, Default)]
pub struct NoopPortProbe;

impl PortProbe for NoopPortProbe {
    fn listening_ports(&self, _groups: &[i32]) -> HashMap<i32, Vec<u16>> {
        HashMap::new()
    }
}
