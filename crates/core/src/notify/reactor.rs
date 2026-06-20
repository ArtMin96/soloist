//! The notification reactor: turns the events that warrant the user's attention into desktop
//! toasts.
//!
//! It subscribes to the event bus and, for a crash or an exhausted auto-restart, composes a
//! [`Notification`] and hands it to the [`Notifier`] port. It resolves each process's label
//! from the supervisor read model, holds a [`Weak`] reference so it never keeps the app alive,
//! and ends when the bus closes (app shutdown) — mirroring the other reactors. A global
//! on/off (a shared flag) gates every toast, so disabling notifications silences them at the
//! source rather than in the adapter.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::process::ProcStatus;
use crate::supervisor::Supervisor;

use super::notifier::{Notification, Notifier};

/// Shows a desktop toast for the attention-worthy events. Built once by the composition root
/// (via [`crate::facade::Facade::notifications_loop`]) and spawned on the runtime.
pub struct NotificationReactor {
    notifier: Arc<dyn Notifier>,
    enabled: Arc<AtomicBool>,
    events: broadcast::Receiver<DomainEvent>,
    supervisor: Weak<Supervisor>,
}

impl NotificationReactor {
    /// Builds a reactor over the notifier and the shared on/off flag, watching the supervisor
    /// weakly (so it never keeps the app alive) and subscribing to the bus.
    pub fn new(
        notifier: Arc<dyn Notifier>,
        enabled: Arc<AtomicBool>,
        bus: &EventBus,
        supervisor: Weak<Supervisor>,
    ) -> Self {
        Self {
            notifier,
            enabled,
            events: bus.subscribe(),
            supervisor,
        }
    }

    /// Runs until the bus closes (app shutdown). Each attention-worthy event becomes one
    /// toast, unless notifications are disabled. A lagged subscriber simply misses a toast
    /// (best-effort) rather than re-syncing — a notification is a transient signal, not state.
    pub async fn run(mut self) {
        loop {
            match self.events.recv().await {
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(_)) => continue,
                Ok(event) => {
                    if !self.enabled.load(Ordering::Relaxed) {
                        continue;
                    }
                    if let Some(notification) = self.compose(&event) {
                        self.notifier.notify(notification);
                    }
                }
            }
        }
    }

    /// The toast a given event warrants, or `None` if it needs none. For v1 the attention
    /// events are a crash and an exhausted auto-restart; both resolve the process label, so a
    /// process gone from the registry yields no toast.
    fn compose(&self, event: &DomainEvent) -> Option<Notification> {
        match event {
            DomainEvent::ProcessStatusChanged {
                id,
                to: ProcStatus::Crashed,
                ..
            } => Some(Notification {
                title: format!("{} crashed", self.label_of(*id)?),
                body: "The process exited unexpectedly.".into(),
            }),
            DomainEvent::RestartExhausted { id } => Some(Notification {
                title: format!("{} stopped", self.label_of(*id)?),
                body: "Auto-restart gave up after too many crashes.".into(),
            }),
            _ => None,
        }
    }

    /// The display label of a process, or `None` if it (or the supervisor) is gone.
    fn label_of(&self, id: ProcessId) -> Option<String> {
        self.supervisor.upgrade()?.label_of(id)
    }
}

#[cfg(test)]
#[path = "reactor_tests.rs"]
mod tests;
