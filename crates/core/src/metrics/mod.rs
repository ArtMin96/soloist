//! Process monitoring (context C5): per-process CPU and memory sampling.
//!
//! This domain owns everything about *how metrics work* — the data it reports
//! ([`ProcessMetrics`]), the OS read it depends on ([`MetricsProbe`], a port this context
//! defines for itself), and the policy that drives sampling ([`MetricsSampler`]). Adding a
//! new kind of metric is a change here, never in the shared port layer. The OS read is an
//! adapter (`crates/sys`); a missing adapter degrades to [`NoopMetricsProbe`].

mod probe;
mod sampler;

pub use probe::{MetricsProbe, NoopMetricsProbe, ProcessMetrics};
pub use sampler::MetricsSampler;
