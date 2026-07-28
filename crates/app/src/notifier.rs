//! The desktop-notification adapter: implements the core's [`Notifier`] over the Tauri
//! notification plugin.
//!
//! The notification reactor (core C7) decides when to notify and composes the toast; this
//! adapter only renders it via the plugin. Best-effort by contract: a failed toast is dropped,
//! never propagated — a notification can never block or crash the core.
//!
//! The status probe goes straight to the plugin's own D-Bus client instead, since the plugin
//! surfaces no way to ask whether a notification backend is listening.

use notify_rust::{get_capabilities, get_server_information};
use soloist_core::{Notification, Notifier, NotifierStatus};
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
        let mut builder = self
            .app
            .notification()
            .builder()
            .title(notification.title)
            .body(notification.body);
        if let Some(sound) = notification.sound {
            builder = builder.sound(sound);
        }
        let _ = builder.show();
    }

    /// Asks the desktop's notification backend to describe itself. Both halves must answer for
    /// the channel to count as available: a backend that names itself but cannot list what it
    /// supports has told us too little to report honestly. Any failure — no session bus, no
    /// backend, a protocol error — is [`NotifierStatus::Unavailable`], never a panic.
    ///
    /// Each half is a blocking round trip, so this belongs on a user action (opening the
    /// settings surface, sending a test toast), never on the event path.
    fn status(&self) -> NotifierStatus {
        let Ok(server) = get_server_information() else {
            return NotifierStatus::Unavailable;
        };
        let Ok(capabilities) = get_capabilities() else {
            return NotifierStatus::Unavailable;
        };
        NotifierStatus::Available {
            server: server.name,
            version: server.version,
            capabilities,
        }
    }
}
