//! The notification reactor: turns the events that warrant the user's attention into a desktop
//! notification, an in-app toast, or nothing.
//!
//! It subscribes to the event bus and, for each attention-worthy event, resolves the originating
//! process's project and label from the supervisor read model, then walks one decision path:
//! the global master switch, the crashed command's own restart policy, the notification level in
//! force for that command, and finally [`route`] — which turns where the user is into which
//! surface the alert reaches. Two of the gates read live from the durable settings so a change
//! takes effect at once: the master switch (global settings), then the level — the project's,
//! tightened by any per-command override — which admits the signal or drops it by its severity.
//! The restart-policy gate reads the command's declared policy: a command that heals itself
//! retries silently, so only its giving up is announced.
//!
//! Anything delivered is also recorded as unread against its process, and a process leaving the
//! registry takes its unread with it. It holds a [`Weak`] reference to the supervisor so it never
//! keeps the app alive, and ends when the bus closes (app shutdown), mirroring the other reactors.

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

use super::attention::AttentionRegistry;
use super::notifier::{Notification, Notifier};
use super::presence::PresenceCell;
use super::routing::{route, Delivery};

/// Delivers the attention-worthy events to whichever surface the user can actually see. Built once
/// by the composition root (via [`crate::facade::Facade::notifications_loop`]) and spawned on the
/// runtime.
pub struct NotificationReactor {
    notifier: Arc<dyn Notifier>,
    global_settings: Arc<SettingsStore<(), Settings>>,
    project_settings: Arc<SettingsStore<ProjectId, ProjectSettings>>,
    presence: Arc<PresenceCell>,
    attention: Arc<AttentionRegistry>,
    bus: EventBus,
    events: broadcast::Receiver<DomainEvent>,
    supervisor: Weak<Supervisor>,
}

impl NotificationReactor {
    /// Builds a reactor over the notifier, the settings stores it gates on, and the presence and
    /// unread state it routes by, watching the supervisor weakly (so it never keeps the app alive)
    /// and subscribing to the bus.
    pub fn new(
        notifier: Arc<dyn Notifier>,
        global_settings: Arc<SettingsStore<(), Settings>>,
        project_settings: Arc<SettingsStore<ProjectId, ProjectSettings>>,
        presence: Arc<PresenceCell>,
        attention: Arc<AttentionRegistry>,
        bus: &EventBus,
        supervisor: Weak<Supervisor>,
    ) -> Self {
        Self {
            notifier,
            global_settings,
            project_settings,
            presence,
            attention,
            bus: bus.clone(),
            events: bus.subscribe(),
            supervisor,
        }
    }

    /// Runs until the bus closes (app shutdown). A lagged subscriber simply misses an alert
    /// (best-effort) rather than re-syncing — a notification is a transient signal, not state.
    pub async fn run(mut self) {
        loop {
            match self.events.recv().await {
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(_)) => continue,
                Ok(event) => self.react(&event),
            }
        }
    }

    /// Handles one event: a departing process loses whatever it had unread, and anything else is
    /// routed to the surface the user can see.
    fn react(&self, event: &DomainEvent) {
        if let DomainEvent::ProcessRemoved { id } = event {
            // Nothing can visit a process that no longer exists, so its unread would sit in the
            // count forever with no way to clear it.
            self.clear_unread(*id);
            return;
        }
        let Some(alert) = self.decide(event) else {
            return;
        };
        match alert.delivery {
            // The user watched it happen; telling them again is noise, and marking it unread
            // would leave them to dismiss something they have already seen.
            Delivery::Suppressed => return,
            Delivery::Native => self.notifier.notify(alert.notification),
            Delivery::Toast => self.bus.publish(DomainEvent::NotificationRaised {
                process: alert.process,
                kind: alert.kind,
                title: alert.notification.title,
                body: alert.notification.body,
                sound: alert.notification.sound,
            }),
        }
        self.attention.raise(alert.process, alert.kind);
        self.bus.publish(DomainEvent::AttentionChanged);
    }

    /// The alert a given event warrants, or `None` if it warrants none. A non-attention event, a
    /// globally-disabled notifier, a process gone from the registry, or a crash the command's own
    /// restart policy will retry each yields nothing at all. Settings read as their documented
    /// defaults (level `All`) on a read error, so a transient store failure never swallows a crash
    /// alert.
    fn decide(&self, event: &DomainEvent) -> Option<Alert> {
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
        let level = settings.effective_level_for(&view.label);
        Some(Alert {
            process: id,
            kind,
            delivery: route(id, kind, self.presence.get(), level),
            notification: notification(kind, &view.label),
        })
    }

    /// Forgets what `id` had unread, announcing the change only when there was something to
    /// forget.
    fn clear_unread(&self, id: ProcessId) {
        if self.attention.clear(id) {
            self.bus.publish(DomainEvent::AttentionChanged);
        }
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

/// One alert that survived every gate: what to say, about which process, and where to say it.
/// The text is composed once here, so the desktop and the in-app toast can never word the same
/// event differently.
struct Alert {
    process: ProcessId,
    kind: AttentionKind,
    delivery: Delivery,
    notification: Notification,
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

/// The alert text this kind shows for the named process — the one place a title and body are
/// written, whichever surface ends up rendering them.
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
