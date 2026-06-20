//! The desktop-notification adapter: implements the core's [`Notifier`] over the Tauri
//! notification plugin.
//!
//! The notification reactor (core C7) decides when to notify and composes the toast; this
//! adapter only renders it via the plugin. Best-effort by contract: a failed toast is dropped,
//! never propagated — a notification can never block or crash the core.

use soloist_core::{Notification, Notifier};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Shows desktop notifications through the Tauri notification plugin. Holds an [`AppHandle`],
/// so it is constructed in the composition root once the app exists.
pub struct TauriNotifier {
    app: AppHandle,
}

impl TauriNotifier {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl Notifier for TauriNotifier {
    fn notify(&self, notification: Notification) {
        let _ = self
            .app
            .notification()
            .builder()
            .title(notification.title)
            .body(notification.body)
            .show();
    }
}
