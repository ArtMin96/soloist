//! The core port set: a parameter object bundling the port adapters the core is
//! constructed over, kept separate from the port *traits* (`super`) so each file has
//! one purpose.

use std::sync::Arc;

use crate::metrics::{MetricsProbe, NoopMetricsProbe};
use crate::portscan::{NoopPortProbe, PortProbe};

use super::{
    Clock, LockReleaser, NoopLockReleaser, NoopOrphanControl, NoopRuntimeState, OrphanControl,
    ProcessSpawner, ProjectRepo, RuntimeState, TrustRepo,
};

/// The set of port adapters the core is constructed over — a parameter object so the
/// core's constructors take one value, and adding a future port is one field here
/// rather than another argument threaded through every call site. The required adapters
/// (`spawner`, `clock`, `trust`, `projects`) have no meaningful absence; the optional
/// driven subsystems (`locks`, `runtime`, `orphan_control`, `metrics`, `port_probe`)
/// default to their `Noop` port via [`CorePorts::builder`], so a new optional port never
/// forces every existing composition root to change. The composition root
/// (`app::build_facade`) is the one place these are chosen; tests assemble it from
/// `crate::testing` fakes.
pub struct CorePorts {
    pub(crate) spawner: Arc<dyn ProcessSpawner>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) trust: Arc<dyn TrustRepo>,
    pub(crate) projects: Arc<dyn ProjectRepo>,
    pub(crate) locks: Arc<dyn LockReleaser>,
    pub(crate) runtime: Arc<dyn RuntimeState>,
    pub(crate) orphan_control: Arc<dyn OrphanControl>,
    pub(crate) metrics: Arc<dyn MetricsProbe>,
    pub(crate) port_probe: Arc<dyn PortProbe>,
}

impl CorePorts {
    /// Begins a port set with the required adapters; the optional driven subsystems
    /// default to their `Noop` port until overridden on the returned builder.
    pub fn builder(
        spawner: Arc<dyn ProcessSpawner>,
        clock: Arc<dyn Clock>,
        trust: Arc<dyn TrustRepo>,
        projects: Arc<dyn ProjectRepo>,
    ) -> CorePortsBuilder {
        CorePortsBuilder {
            ports: CorePorts {
                spawner,
                clock,
                trust,
                projects,
                locks: Arc::new(NoopLockReleaser),
                runtime: Arc::new(NoopRuntimeState),
                orphan_control: Arc::new(NoopOrphanControl),
                metrics: Arc::new(NoopMetricsProbe),
                port_probe: Arc::new(NoopPortProbe),
            },
        }
    }
}

/// Builder for [`CorePorts`]: override the optional driven subsystems, then `build`.
pub struct CorePortsBuilder {
    ports: CorePorts,
}

impl CorePortsBuilder {
    /// Overrides the lock releaser (coordination C6; defaults to [`NoopLockReleaser`]).
    pub fn locks(mut self, locks: Arc<dyn LockReleaser>) -> Self {
        self.ports.locks = locks;
        self
    }

    /// Overrides the runtime-state recorder for orphan adoption (defaults to
    /// [`NoopRuntimeState`]).
    pub fn runtime(mut self, runtime: Arc<dyn RuntimeState>) -> Self {
        self.ports.runtime = runtime;
        self
    }

    /// Overrides the orphan group control for adoption (defaults to
    /// [`NoopOrphanControl`]).
    pub fn orphan_control(mut self, orphan_control: Arc<dyn OrphanControl>) -> Self {
        self.ports.orphan_control = orphan_control;
        self
    }

    /// Overrides the CPU/memory probe the metrics sampler reads (monitoring C5; defaults
    /// to [`NoopMetricsProbe`], which produces no readings).
    pub fn metrics(mut self, metrics: Arc<dyn MetricsProbe>) -> Self {
        self.ports.metrics = metrics;
        self
    }

    /// Overrides the port probe the port scanner reads (monitoring C5; defaults to
    /// [`NoopPortProbe`], which discovers nothing).
    pub fn port_probe(mut self, port_probe: Arc<dyn PortProbe>) -> Self {
        self.ports.port_probe = port_probe;
        self
    }

    /// Finishes the port set.
    pub fn build(self) -> CorePorts {
        self.ports
    }
}
