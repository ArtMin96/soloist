//! Presence and unread commands: the shell reports where the user is, and reads what they have
//! not looked at yet.
//!
//! Thin wrappers like the rest of the command surface. The window and the selected process are
//! things only the shell can observe, so it reports them; every decision that follows — which
//! surface an alert reaches, what a sighting clears — belongs to the core.

use std::sync::Arc;

use soloist_core::{AttentionSnapshot, Facade, NotifierStatus, Presence, ProcessId};
use tauri::State;

/// Reports where the user is. Called when the window gains or loses focus and when the selected
/// process changes; seeing a process is what clears its unread mark, so this is a command with an
/// effect rather than a setter.
#[tauri::command]
pub async fn set_presence(
    presence: Presence,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    facade.set_presence(presence);
    Ok(())
}

/// Everything currently unread — the snapshot half of snapshot-then-deltas for attention, paired
/// with the `AttentionChanged` event.
#[tauri::command]
pub async fn attention_snapshot(
    facade: State<'_, Arc<Facade>>,
) -> Result<AttentionSnapshot, String> {
    Ok(facade.attention_snapshot())
}

/// Dismisses what one process had unread.
#[tauri::command]
pub async fn clear_attention(
    process: ProcessId,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    facade.clear_attention(process);
    Ok(())
}

/// Dismisses everything unread.
#[tauri::command]
pub async fn clear_all_attention(facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    facade.clear_all_attention();
    Ok(())
}

/// Shows a sample desktop notification, so a user can tell whether alerts reach them at all.
#[tauri::command]
pub async fn send_test_notification(facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    facade.send_test_notification();
    Ok(())
}

/// What the desktop notification channel can currently do on this machine. Probed on the user's
/// action (opening the Notifications settings, or asking again), never on an interval: the probe
/// is a blocking round trip to the desktop's notification backend.
#[tauri::command]
pub async fn notifier_status(facade: State<'_, Arc<Facade>>) -> Result<NotifierStatus, String> {
    Ok(facade.notifier_status())
}
