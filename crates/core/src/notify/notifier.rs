//! The notifications domain's own driven port: best-effort desktop toasts.
//!
//! The reactor ([`super::NotificationReactor`]) decides *when* and composes *what* to show;
//! the adapter only renders the toast. The real adapter lives in the Tauri shell (it shows
//! the toast via the desktop notification plugin), never in the pure core.

/// A desktop notification to show: a short title line and a longer body line. The domain
/// composes these from a [`crate::events::DomainEvent`]; the adapter just renders them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
}

/// Shows best-effort desktop notifications. An implementation must never block or panic the
/// core: a missing or failing notification backend degrades silently (the toast is dropped),
/// so notifications can never take down the supervisor (the graceful-degradation contract).
pub trait Notifier: Send + Sync {
    /// Shows `notification`. Fire-and-forget and best-effort.
    fn notify(&self, notification: Notification);
}

/// A [`Notifier`] that shows nothing — the default until the desktop adapter is wired
/// (headless tools, tests). With it, the reactor composes notifications but none are shown.
#[derive(Clone, Copy, Default)]
pub struct NoopNotifier;

impl Notifier for NoopNotifier {
    fn notify(&self, _notification: Notification) {}
}
