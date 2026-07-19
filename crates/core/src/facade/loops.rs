//! The long-lived background loops the composition root spawns once on its runtime (context C8).
//!
//! Each method assembles a self-supervised reactor or sampler from the façade's ports and returns
//! its future for the composition root to spawn — it never starts a task itself. Every loop watches
//! the supervisor weakly, so it ends when the façade is dropped, and degrades to a no-op under its
//! `Noop` port when the real adapter is not wired. Grouped here so the façade root stays the public
//! command/query surface.

use std::future::Future;
use std::sync::Arc;

use super::Facade;
use crate::agents::IdleSampler;
use crate::coordination::TemplateEvictor;
use crate::filewatch::WatchReactor;
use crate::metrics::MetricsSampler;
use crate::notify::NotificationReactor;
use crate::portscan::PortScanner;
use crate::projects::ConfigWatchReactor;

impl Facade {
    /// The self-healing reactor loop (crash auto-restart, C2), returned for the
    /// composition root to spawn once on its runtime. It runs until the facade is
    /// dropped; the supervisor's restart policy drives it.
    pub fn self_healing_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        self.supervisor.self_healing_loop()
    }

    /// The metrics sampler loop (monitoring C5), returned for the composition root to spawn
    /// once on its runtime. It samples each running process group on an interval and publishes a
    /// [`crate::events::DomainEvent::MetricsTick`] when a group's reading changes — with an
    /// occasional heartbeat so a subscriber that mounts after the reading last moved still
    /// repopulates — watching the supervisor weakly so it ends when the facade is dropped.
    /// Self-supervised: a panicking sample is isolated and the loop restarts. With the default
    /// [`crate::metrics::NoopMetricsProbe`] it emits nothing — the real CPU/memory adapter is
    /// chosen in the composition root.
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

    /// The config-watch reactor loop (projects C1), returned for the composition root to spawn
    /// once on its runtime. It holds a non-recursive watch on each open project's root and,
    /// on an external `solo.yml` edit, reloads that project (debounced) via the projects
    /// domain — the same reconcile an explicit reload drives, announcing
    /// [`crate::events::DomainEvent::ConfigChanged`] with its trust review. Watches the
    /// supervisor weakly and ends when the bus closes (app shutdown). With the default
    /// [`crate::filewatch::NoopFileWatcher`] it watches nothing, so the real `notify`
    /// adapter is chosen in the composition root.
    pub fn config_watch_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        ConfigWatchReactor::new(
            self.clock.clone(),
            self.file_watcher.clone(),
            &self.bus,
            Arc::downgrade(&self.supervisor),
            self.projects.clone(),
            self.config.clone(),
        )
        .run()
    }

    /// The template cache eviction loop (coordination C6), returned for the composition root to
    /// spawn once on its runtime. Removing a project cascades its template rows away in the store
    /// without the template aggregate seeing a write, so this drops that project's cached entries —
    /// otherwise a later read of the same scope would serve the removed project's rows. Holds the
    /// aggregate weakly and ends when the bus closes (app shutdown); self-supervised like the
    /// samplers.
    pub fn template_eviction_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        TemplateEvictor::new(&self.templates, &self.bus, self.clock.clone()).run()
    }

    /// The notification reactor loop (notifications C7), returned for the composition root to
    /// spawn once on its runtime. It shows a desktop toast for the attention-worthy events,
    /// honouring the global master switch and the per-project alert switches (read live from
    /// settings), watching the supervisor weakly so it ends when the facade is dropped. With the
    /// default [`crate::notify::NoopNotifier`] it shows nothing — the real desktop adapter is
    /// chosen in the composition root.
    pub fn notifications_loop(&self) -> impl Future<Output = ()> + Send + 'static {
        NotificationReactor::new(
            self.notifier.clone(),
            self.settings.clone(),
            self.project_settings.clone(),
            &self.bus,
            Arc::downgrade(&self.supervisor),
        )
        .run()
    }
}
