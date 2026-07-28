//! The notification reactor: turns the events that warrant the user's attention into desktop
//! toasts.
//!
//! It subscribes to the event bus and, for each attention-worthy event, resolves the originating
//! process's project and label from the supervisor read model, then consults the settings before
//! composing a [`Notification`] for the [`Notifier`] port. Two of the gates read live from the
//! durable settings so a change takes effect at once: the global master switch (global settings),
//! then the notification level in force for that command — the project's, tightened by any
//! per-command override — which admits the signal or drops it by its severity. The third reads the
//! crashed command's own restart policy: a command that heals itself retries silently, so only its
//! giving up is announced. It holds a [`Weak`] reference to the supervisor so it never keeps the
//! app alive, and ends when the bus closes (app shutdown), mirroring the other reactors.

use std::sync::{Arc, Weak};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::agents::AgentActivity;
use crate::attention::AttentionKind;
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
    /// non-attention event, a globally-disabled notifier, a process gone from the registry, a crash
    /// the command's own restart policy will retry, or a notification level that does not admit the
    /// signal's severity each yields no toast. Settings read as their documented defaults (level
    /// `All`) on a read error, so a transient store failure never swallows a crash alert.
    fn compose(&self, event: &DomainEvent) -> Option<Notification> {
        let (id, kind) = classify(event)?;
        if !self.globally_enabled() {
            return None;
        }
        let supervisor = self.supervisor.upgrade()?;
        let view = supervisor.view(id)?;
        if !survives_retry_policy(kind, supervisor.auto_restarts(id)) {
            return None;
        }
        let settings = self.project_settings.get(&view.project).unwrap_or_default();
        permitted_by(kind, &settings, &view.label).then(|| notification(kind, &view.label))
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

/// The attention kind a raw event carries, with the process it concerns — or `None` when the
/// event warrants no notification.
fn classify(event: &DomainEvent) -> Option<(ProcessId, AttentionKind)> {
    match event {
        DomainEvent::ProcessStatusChanged {
            id,
            to: ProcStatus::Crashed,
            ..
        } => Some((*id, AttentionKind::Crashed)),
        DomainEvent::RestartExhausted { id } => Some((*id, AttentionKind::RestartExhausted)),
        DomainEvent::AgentActivityChanged {
            id,
            state: AgentActivity::Permission,
        } => Some((*id, AttentionKind::AgentPermission)),
        DomainEvent::AgentActivityChanged {
            id,
            state: AgentActivity::Error,
        } => Some((*id, AttentionKind::AgentError)),
        DomainEvent::TerminalBell { id } => Some((*id, AttentionKind::TerminalBell)),
        _ => None,
    }
}

/// Whether this alert survives the crashed command's restart policy. An `auto_restart` command is
/// relaunched after each crash, so announcing those crashes would fire one toast per attempt — up
/// to the rate limit — for a command that is healing itself; the user learns of it once, when the
/// policy gives up ([`AttentionKind::RestartExhausted`]). The gate reads the declared policy, not
/// the process's current state: at the moment a crash is classified nothing is scheduled yet,
/// because the actor publishes the crash before the self-healing loop consults the policy.
fn survives_retry_policy(kind: AttentionKind, auto_restart: bool) -> bool {
    match kind {
        AttentionKind::Crashed => !auto_restart,
        AttentionKind::RestartExhausted
        | AttentionKind::AgentPermission
        | AttentionKind::AgentError
        | AttentionKind::TerminalBell => true,
    }
}

/// Whether this project's settings permit this alert: the level in force for the command — the
/// project's, tightened by any per-command override — decides by the kind's severity.
fn permitted_by(kind: AttentionKind, settings: &ProjectSettings, label: &str) -> bool {
    settings.effective_level_for(label).admits(kind.severity())
}

/// The toast this kind shows for the named process.
fn notification(kind: AttentionKind, label: &str) -> Notification {
    let (title, body) = match kind {
        AttentionKind::Crashed => (
            format!("{label} crashed"),
            "The process exited unexpectedly.",
        ),
        AttentionKind::RestartExhausted => (
            format!("{label} stopped"),
            "Auto-restart gave up after too many crashes.",
        ),
        AttentionKind::AgentPermission => (
            format!("{label} needs your input"),
            "The agent is waiting for permission.",
        ),
        AttentionKind::AgentError => (
            format!("{label} hit an error"),
            "The agent reported an error.",
        ),
        AttentionKind::TerminalBell => (
            format!("{label} rang the bell"),
            "The terminal signalled for your attention.",
        ),
    };
    Notification {
        title,
        body: body.into(),
        sound: None,
    }
}

#[cfg(test)]
#[path = "reactor_tests.rs"]
mod tests;
