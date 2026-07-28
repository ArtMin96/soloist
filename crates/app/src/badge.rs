//! The app-icon badge: how many alerts are waiting, drawn on Soloist's dock icon for the times
//! when its window is not what the user is looking at.
//!
//! It keeps no count of its own. It reads the same [`Facade::attention_snapshot`] the in-window
//! markers render, so the badge and the title-bar count cannot disagree. Clearing on focus is this
//! surface's own rule rather than the core forgetting: the badge answers "did anything happen while
//! I was away", which is nothing once the user is back, while the in-window markers have to survive
//! that arrival to be findable at all.
//!
//! **On Linux the badge appears only where `libunity` is installed and running.** `tao` `dlopen`s
//! `libunity.so.4`, `.6` or `.9` and returns early unless `unity_inspector_get_unity_running`
//! reports true, and Tauri keys the entry to `<productName>.desktop` — so it also needs the
//! installed package rather than a dev build under another identifier. Everywhere else
//! `set_badge_count` is a silent no-op: no error, no log, nothing to chase.

use std::sync::Arc;

use soloist_core::events::DomainEvent;
use soloist_core::facade::Facade;
use soloist_core::notify::{AttentionSnapshot, Presence};
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast::{error::RecvError, Receiver};

use crate::MAIN_WINDOW;

/// The largest number the badge draws. The snapshot keeps the true total for the in-window count,
/// which is the surface with room to say "99+".
const BADGE_CAP: usize = 99;

/// Starts the badge reactor. This call and this file are the whole feature — nothing else refers
/// to it.
pub fn install(app: &AppHandle) {
    let facade = Arc::clone(app.state::<Arc<Facade>>().inner());
    let events = facade.subscribe();
    let app = app.clone();
    tauri::async_runtime::spawn(run(
        events,
        move || (facade.presence(), facade.attention_snapshot()),
        move |count| {
            // Resolved per update rather than held: the window can be gone by the time an event
            // arrives, and a badge is not a reason to keep a destroyed one alive.
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                let _ = window.set_badge_count(count);
            }
        },
    ));
}

/// Keeps the badge in step with what is unread and where the user is, ending when the bus closes.
async fn run(
    mut events: Receiver<DomainEvent>,
    state: impl Fn() -> (Presence, AttentionSnapshot),
    set_badge: impl Fn(Option<i64>),
) {
    loop {
        match events.recv().await {
            // Falling behind costs nothing here: the count is re-read rather than folded from the
            // deltas, so a lag is caught up by the same recompute a change asks for.
            Ok(DomainEvent::AttentionChanged | DomainEvent::PresenceChanged)
            | Err(RecvError::Lagged(_)) => {
                let (presence, snapshot) = state();
                set_badge(count(presence, &snapshot));
            }
            Ok(_) => {}
            Err(RecvError::Closed) => break,
        }
    }
}

/// What the badge should show: nothing while the user is at the window or nothing is waiting,
/// otherwise how many alerts are, capped for display.
///
/// Empty reads as `None` rather than `Some(0)` because some docks keep drawing a zero they were
/// handed, leaving a badge saying nothing is waiting instead of no badge at all.
fn count(presence: Presence, snapshot: &AttentionSnapshot) -> Option<i64> {
    if presence.focused || snapshot.total == 0 {
        return None;
    }
    Some(snapshot.total.min(BADGE_CAP) as i64)
}

#[cfg(test)]
#[path = "badge_tests.rs"]
mod tests;
