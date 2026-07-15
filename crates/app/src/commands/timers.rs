//! Timer management commands — the local, trusted Tauri surface for the timers panel.
//!
//! Each command routes to the one core behaviour via the `*_for` façade methods, which take an
//! explicit `owner: ProcessId` rather than resolving it from a session (the local UI is trusted and
//! already holds the agent tree). No logic lives here: the core enforces owner-scoping (a caller
//! cannot cancel another agent's timer), emits the matching `DomainEvent`, and returns whether one
//! was affected.

use std::sync::Arc;

use soloist_core::{Facade, ProcessId, TimerId};
use tauri::State;

/// Cancels a timer owned by `owner`. Returns `true` if the timer existed and was removed;
/// `false` if it was already gone or `owner` does not hold it (another process's timer).
#[tauri::command]
pub async fn timer_cancel(
    owner: u64,
    timer: u64,
    facade: State<'_, Arc<Facade>>,
) -> Result<bool, String> {
    facade
        .blocking(move |f| f.timer_cancel_for(ProcessId::from_raw(owner), TimerId::from_raw(timer)))
        .await
        .map_err(|err| err.to_string())
}

/// Pauses a timer owned by `owner` (freezes the remaining time). Returns `true` if the timer
/// was armed and has been paused; `false` if it was already paused or gone.
#[tauri::command]
pub async fn timer_pause(
    owner: u64,
    timer: u64,
    facade: State<'_, Arc<Facade>>,
) -> Result<bool, String> {
    facade
        .blocking(move |f| f.timer_pause_for(ProcessId::from_raw(owner), TimerId::from_raw(timer)))
        .await
        .map_err(|err| err.to_string())
}

/// Resumes a paused timer owned by `owner` (re-arms it with the time that remained). Returns
/// `true` if the timer was paused and has been resumed; `false` if it was already armed or gone.
#[tauri::command]
pub async fn timer_resume(
    owner: u64,
    timer: u64,
    facade: State<'_, Arc<Facade>>,
) -> Result<bool, String> {
    facade
        .blocking(move |f| f.timer_resume_for(ProcessId::from_raw(owner), TimerId::from_raw(timer)))
        .await
        .map_err(|err| err.to_string())
}
