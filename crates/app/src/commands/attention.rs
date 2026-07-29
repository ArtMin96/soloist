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
///
/// Routed through the blocking pool because composing it reads the stored bell from the durable
/// settings; showing the notification itself is fire-and-forget.
#[tauri::command]
pub async fn send_test_notification(facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    let facade = Arc::clone(&facade);
    facade.blocking(|f| f.send_test_notification()).await;
    Ok(())
}

/// What the desktop notification channel can currently do on this machine. Probed on the user's
/// action (opening the Notifications settings, or asking again), never on an interval.
///
/// Routed through the blocking pool because the probe is two synchronous D-Bus round trips: a
/// session bus that is slow or wedged would otherwise park a runtime worker, stalling the commands
/// scheduled behind it for a question nobody is waiting on.
#[tauri::command]
pub async fn notifier_status(facade: State<'_, Arc<Facade>>) -> Result<NotifierStatus, String> {
    let facade = Arc::clone(&facade);
    Ok(facade.blocking(|f| f.notifier_status()).await)
}
