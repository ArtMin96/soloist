//! The notifications domain's own driven port: best-effort desktop toasts.
//!
//! The reactor ([`super::NotificationReactor`]) decides *when* and composes *what* to show;
//! the adapter only renders the toast. The real adapter lives in the Tauri shell (it shows
//! the toast via the desktop notification plugin), never in the pure core.

use serde::Serialize;

/// A desktop notification to show: a short title line and a longer body line. The domain
/// composes these from a [`crate::events::DomainEvent`]; the adapter just renders them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
    /// The name of a sound for the backend to play, or `None` to show silently. Only a hint:
    /// an unrecognised name is the backend's to ignore, and the notification still shows, so
    /// the domain never validates it against what the backend advertises.
    pub sound: Option<String>,
}

/// What the desktop notification channel can currently do on this machine.
///
/// This describes the channel, never an individual toast: the channel is fire-and-forget, so
/// whether a notification actually reached the user is not observable and must never be
/// presented as confirmed. `Available` means something is listening — no more than that.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotifierStatus {
    /// Nothing is listening, so a toast goes nowhere. The default, because until a probe says
    /// otherwise the safe answer is that the channel cannot deliver rather than that it can.
    #[default]
    Unavailable,
    /// A notification backend is listening, reporting its own name and version and the
    /// capability names it advertises.
    Available {
        server: String,
        version: String,
        capabilities: Vec<String>,
    },
}

/// Shows best-effort desktop notifications. An implementation must never block or panic the
/// core: a missing or failing notification backend degrades silently (the toast is dropped),
/// so notifications can never take down the supervisor (the graceful-degradation contract).
pub trait Notifier: Send + Sync {
    /// Shows `notification`. Fire-and-forget and best-effort.
    fn notify(&self, notification: Notification);

    /// Reports what the channel can currently do, so a caller can tell the user why no toast
    /// appeared. Answered on demand rather than cached: a backend can start or stop while the
    /// app runs, so a remembered answer would go stale silently. Bound by the same contract as
    /// [`Notifier::notify`] — an unreachable backend is [`NotifierStatus::Unavailable`], never
    /// an error to propagate or a panic.
    fn status(&self) -> NotifierStatus;
}

/// A [`Notifier`] that shows nothing — the default until the desktop adapter is wired
/// (headless tools, tests). With it, the reactor composes notifications but none are shown.
#[derive(Clone, Copy, Default)]
pub struct NoopNotifier;

impl Notifier for NoopNotifier {
    fn notify(&self, _notification: Notification) {}

    /// Always unavailable: there is no backend behind this implementation, so it genuinely
    /// cannot deliver anything and should not claim it can.
    fn status(&self) -> NotifierStatus {
        NotifierStatus::Unavailable
    }
}

#[cfg(test)]
#[path = "notifier_tests.rs"]
mod tests;
