//! The long-lived background loops the composition root spawns once on its runtime (context C8).
//!
//! Each method assembles a self-supervised reactor or sampler from the façade's ports and returns
//! its future for the composition root to spawn — it never starts a task itself. Every loop watches
//! the supervisor weakly, so it ends when the façade is dropped, and degrades to a no-op under its
//! `Noop` port when the real adapter is not wired. Grouped here so the façade root stays the public
//! command/query surface.

use std::future::Future;
use std::sync::{Arc, Weak};

use super::Facade;
use crate::agents::{IdleSampler, OutputSnapshot, SummaryReactor};
use crate::filewatch::WatchReactor;
use crate::ids::ProcessId;
use crate::metrics::MetricsSampler;
use crate::notify::NotificationReactor;
use crate::portscan::PortScanner;
use crate::supervisor::Supervisor;

impl Facade {
    /// The self-healing reactor loop (crash auto-restart, C2), returned for the
    /// composition root to spawn once on its runtime. It runs until the facade is
    /// dropped; the supervisor's restart policy drives it.
    pub fn self_healing_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        self.supervisor.self_healing_loop()
    }

    /// The metrics sampler loop (monitoring C5), returned for the composition root to spawn
    /// once on its runtime. It samples each running process group on an interval and
    /// publishes a [`crate::events::DomainEvent::MetricsTick`] per group, watching the supervisor
    /// weakly so it ends when the facade is dropped. Self-supervised: a panicking sample is isolated
    /// and the loop restarts. With the default [`crate::metrics::NoopMetricsProbe`] it emits
    /// nothing — the real CPU/memory adapter is chosen in the composition root.
    pub fn metrics_sampler_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        MetricsSampler::new(
            self.clock.clone(),
            self.metrics.clone(),
            self.bus.clone(),
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }

    /// The agent idle-detection sampler loop (agents C4), returned for the composition root to
    /// spawn once on its runtime. It reclassifies each launched agent on an interval from its
    /// terminal output and publishes a [`crate::events::DomainEvent::AgentActivityChanged`] on a
    /// transition, watching the supervisor weakly so it ends when the facade is dropped.
    /// Self-supervised like the other samplers; agents are registered for tracking by
    /// [`Facade::launch_agent`].
    pub fn idle_sampler_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        IdleSampler::new(
            self.clock.clone(),
            self.idle.clone(),
            self.lineage.clone(),
            self.bus.clone(),
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }

    /// The agent auto-summarization reactor loop (agents C4), returned for the composition root to
    /// spawn once on its runtime. It reacts to an agent going idle by producing a one-line summary
    /// of its recent terminal output through the user's configured summarizer CLI, run headless,
    /// and publishing a [`crate::events::DomainEvent::AgentSummary`]. Optional and OFF by default:
    /// with no summarizer tool configured, an unsupported provider, or the default
    /// [`crate::agents::NoopSummaryRunner`] (no adapter wired), it produces nothing and never
    /// blocks the core — the real headless executor is chosen in the composition root. It reads
    /// recent output from the supervisor weakly so it never keeps the app alive.
    pub fn summary_reactor_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        let snapshots = Arc::new(SupervisorSnapshot(Arc::downgrade(&self.supervisor)));
        SummaryReactor::new(
            self.clock.clone(),
            self.summary_runner.clone(),
            self.settings.repo(),
            self.agents.tool_repo(),
            snapshots,
            self.bus.clone(),
        )
        .run()
    }

    /// The coordination timer scheduler loop (C6), returned for the composition root to spawn
    /// once on its runtime. It fires each due timer — at its deadline, or when the agents it
    /// watches go idle — and delivers the timer's body to its owning process as a fresh turn
    /// (reusing the supervisor's input behaviour). It tracks idle state from the
    /// [`crate::events::DomainEvent::AgentActivityChanged`] stream, watches the supervisor weakly so
    /// it ends when the facade is dropped, and is self-supervised like the samplers. With the
    /// default [`crate::coordination::NoopTimerRepo`] no timer ever persists, so it fires nothing —
    /// the real SQLite store is chosen in the composition root.
    pub fn timer_scheduler_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        self.timers
            .scheduler(self.bus.clone(), Arc::downgrade(&self.supervisor))
            .run()
    }

    /// The port-discovery scanner loop (monitoring C5), returned for the composition root to
    /// spawn once on its runtime. It discovers each running process group's listening ports,
    /// reflects them on [`crate::process::ProcessView::ports`], and publishes
    /// [`crate::events::DomainEvent::PortsChanged`] on a real change. Watches the supervisor weakly
    /// and is self-supervised, like the metrics sampler. With the default
    /// [`crate::portscan::NoopPortProbe`] it finds nothing.
    pub fn port_scanner_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        PortScanner::new(
            self.clock.clone(),
            self.port_probe.clone(),
            self.bus.clone(),
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }

    /// The file-watch reactor loop (monitoring C5), returned for the composition root to spawn
    /// once on its runtime. It watches each trusted, file-watched command's project root and,
    /// on a matching change, restarts that command (debounced) via the supervisor — reusing
    /// one restart behaviour. Watches the supervisor weakly and ends when the bus closes (app
    /// shutdown). With the default [`crate::filewatch::NoopFileWatcher`] it watches nothing,
    /// so the real `notify` adapter is chosen in the composition root.
    pub fn file_watch_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        WatchReactor::new(
            self.clock.clone(),
            self.file_watcher.clone(),
            &self.bus,
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }

    /// The notification reactor loop (notifications C7), returned for the composition root to
    /// spawn once on its runtime. It shows a desktop toast on a crash or an exhausted
    /// auto-restart (honouring the global on/off), watching the supervisor weakly so it ends
    /// when the facade is dropped. With the default [`crate::notify::NoopNotifier`] it shows
    /// nothing — the real desktop adapter is chosen in the composition root.
    pub fn notifications_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        NotificationReactor::new(
            self.notifier.clone(),
            self.notifications_enabled.clone(),
            &self.bus,
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }
}

/// Reads a process's recent rendered output from the supervisor (C2) for the summary reactor (C4),
/// holding it weakly so the reactor never keeps the app alive. The one place the summarization
/// subsystem is bound to the concrete supervisor; the reactor itself depends only on the narrow
/// [`OutputSnapshot`] read.
struct SupervisorSnapshot(Weak<Supervisor>);

impl OutputSnapshot for SupervisorSnapshot {
    fn recent_lines(&self, id: ProcessId, max_lines: usize) -> Vec<String> {
        self.0
            .upgrade()
            .and_then(|supervisor| supervisor.rendered_tail(id, max_lines))
            .unwrap_or_default()
    }
}
