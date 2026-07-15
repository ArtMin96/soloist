//! The notification reactor: turns the events that warrant the user's attention into desktop
//! toasts.
//!
//! It subscribes to the event bus and, for each attention-worthy event, resolves the originating
//! process's project and label from the supervisor read model, then consults the settings before
//! composing a [`Notification`] for the [`Notifier`] port. Three gates apply, all read live from the
//! durable settings so a toggle takes effect at once: the global master switch (global settings),
//! then the per-project switch for that kind of alert — crash/exit alerts for a crash or exhausted
//! restart, terminal alerts (with the per-command override) for a bell or an agent asking for
//! attention. It holds a [`Weak`] reference to the supervisor so it never keeps the app alive, and
//! ends when the bus closes (app shutdown), mirroring the other reactors.

use std::sync::{Arc, Weak};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::agents::AgentActivity;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::process::ProcStatus;
use crate::settings::{ProjectSettings, Settings, SettingsStore};
use crate::supervisor::Supervisor;

use super::notifier::{Notification, Notifier};

/// Shows a desktop toast for the attention-worthy events. Built once by the composition root
/// (via [`crate::facade::Facade::notifications_loop`]) and spawned on the runtime.
pub struct NotificationReactor {
    notifier: Arc<dyn Notifier>,
    global_settings: Arc<SettingsStore<(), Settings>>,
    project_settings: Arc<SettingsStore<ProjectId, ProjectSettings>>,
    events: broadcast::Receiver<DomainEvent>,
    supervisor: Weak<Supervisor>,
}

impl NotificationReactor {
    /// Builds a reactor over the notifier and the settings stores it gates on, watching the
    /// supervisor weakly (so it never keeps the app alive) and subscribing to the bus.
    pub fn new(
        notifier: Arc<dyn Notifier>,
        global_settings: Arc<SettingsStore<(), Settings>>,
        project_settings: Arc<SettingsStore<ProjectId, ProjectSettings>>,
        bus: &EventBus,
        supervisor: Weak<Supervisor>,
    ) -> Self {
        Self {
            notifier,
            global_settings,
            project_settings,
            events: bus.subscribe(),
            supervisor,
        }
    }

    /// Runs until the bus closes (app shutdown). Each attention-worthy event becomes one toast,
    /// unless a gate silences it. A lagged subscriber simply misses a toast (best-effort) rather
    /// than re-syncing — a notification is a transient signal, not state.
    pub async fn run(mut self) {
        loop {
            match self.events.recv().await {
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(_)) => continue,
                Ok(event) => {
                    if let Some(notification) = self.compose(&event) {
                        self.notifier.notify(notification);
                    }
                }
            }
        }
    }

    /// The toast a given event warrants, or `None` if it needs none or a gate silences it. A
    /// non-attention event, a globally-disabled notifier, a process gone from the registry, or the
    /// relevant per-project switch being off each yields no toast. Settings read as their documented
    /// defaults (alerts on) on a read error, so a transient store failure never swallows a crash
    /// alert.
    fn compose(&self, event: &DomainEvent) -> Option<Notification> {
        let (id, attention) = Attention::classify(event)?;
        if !self.globally_enabled() {
            return None;
        }
        let view = self.supervisor.upgrade()?.view(id)?;
        let settings = self.project_settings.get(&view.project).unwrap_or_default();
        attention
            .permitted_by(&settings, &view.label)
            .then(|| attention.notification(&view.label))
    }

    /// Whether the global master switch is on. Reads the durable global settings live so a change
    /// takes effect at once; a read error defaults to on (notifications remain a best-effort signal).
    fn globally_enabled(&self) -> bool {
        self.global_settings
            .get(&())
            .map(|s| s.notifications.enabled)
            .unwrap_or(true)
    }
}

/// The kinds of event that warrant a toast. Each maps to the per-project switch that gates it and
/// the toast it composes, so adding a kind is one variant plus its arm in each exhaustive match.
#[derive(Clone, Copy)]
enum Attention {
    Crashed,
    Exhausted,
    Permission,
    Error,
    Bell,
}

impl Attention {
    /// The attention kind a raw event carries, with the process it concerns — or `None` when the
    /// event warrants no notification.
    fn classify(event: &DomainEvent) -> Option<(ProcessId, Attention)> {
        match event {
            DomainEvent::ProcessStatusChanged {
                id,
                to: ProcStatus::Crashed,
                ..
            } => Some((*id, Attention::Crashed)),
            DomainEvent::RestartExhausted { id } => Some((*id, Attention::Exhausted)),
            DomainEvent::AgentActivityChanged {
                id,
                state: AgentActivity::Permission,
            } => Some((*id, Attention::Permission)),
            DomainEvent::AgentActivityChanged {
                id,
                state: AgentActivity::Error,
            } => Some((*id, Attention::Error)),
            DomainEvent::TerminalBell { id } => Some((*id, Attention::Bell)),
            _ => None,
        }
    }

    /// Whether this project's settings permit this alert: crash/exit alerts gate a crash or an
    /// exhausted restart; terminal alerts (with the per-command override) gate a bell or an agent
    /// asking for attention — the same split the settings UI presents.
    fn permitted_by(self, settings: &ProjectSettings, label: &str) -> bool {
        match self {
            Attention::Crashed | Attention::Exhausted => settings.crash_exit_alerts,
            Attention::Permission | Attention::Error | Attention::Bell => {
                settings.terminal_alerts_for(label)
            }
        }
    }

    /// The toast this kind shows for the named process.
    fn notification(self, label: &str) -> Notification {
        let (title, body) = match self {
            Attention::Crashed => (
                format!("{label} crashed"),
                "The process exited unexpectedly.",
            ),
            Attention::Exhausted => (
                format!("{label} stopped"),
                "Auto-restart gave up after too many crashes.",
            ),
            Attention::Permission => (
                format!("{label} needs your input"),
                "The agent is waiting for permission.",
            ),
            Attention::Error => (
                format!("{label} hit an error"),
                "The agent reported an error.",
            ),
            Attention::Bell => (
                format!("{label} rang the bell"),
                "The terminal signalled for your attention.",
            ),
        };
        Notification {
            title,
            body: body.into(),
        }
    }
}

#[cfg(test)]
#[path = "reactor_tests.rs"]
mod tests;
