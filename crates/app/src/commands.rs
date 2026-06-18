//! The Tauri command surface: thin wrappers that route to the one core behaviour.
//!
//! No business logic lives here. Each command translates webview arguments into a call
//! on the [`Facade`]/supervisor and maps the typed core error to a string the UI can
//! render — so "restart", "start all", and the trust gate are implemented exactly once,
//! in the core. PTY bytes stream over a [`Channel`] (the high-throughput IPC primitive);
//! status and config changes ride the domain-event bus from `lib.rs`.
//!
//! [`Channel`]: tauri::ipc::Channel

use soloist_core::{Facade, ProcessId, ProcessView, ProjectId};
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::broadcast::error::RecvError;

use crate::pty_bridge::PtyBridge;

/// The current process read model — the snapshot half of snapshot-then-deltas.
#[tauri::command]
pub async fn proc_list(facade: State<'_, Facade>) -> Result<Vec<ProcessView>, String> {
    Ok(facade.snapshot())
}

/// Starts one process; refused by the core trust gate if its command is untrusted.
#[tauri::command]
pub async fn proc_start(id: u64, facade: State<'_, Facade>) -> Result<(), String> {
    facade
        .supervisor()
        .start(ProcessId::from_raw(id))
        .map_err(|err| err.to_string())
}

/// Requests a graceful stop of one process; reports whether it was found live.
#[tauri::command]
pub async fn proc_stop(id: u64, facade: State<'_, Facade>) -> Result<bool, String> {
    Ok(facade.supervisor().stop(ProcessId::from_raw(id)))
}

/// Restarts one process (stop then start with its saved config); trust-gated.
#[tauri::command]
pub async fn proc_restart(id: u64, facade: State<'_, Facade>) -> Result<(), String> {
    facade
        .supervisor()
        .restart(ProcessId::from_raw(id))
        .map_err(|err| err.to_string())
}

/// Starts every trusted auto-start command in a project (untrusted ones are skipped).
#[tauri::command]
pub async fn stack_start(project: u64, facade: State<'_, Facade>) -> Result<(), String> {
    facade
        .supervisor()
        .start_all(ProjectId::from_raw(project))
        .map(|_summary| ())
        .map_err(|err| err.to_string())
}

/// Stops every live process in a project.
#[tauri::command]
pub async fn stack_stop(project: u64, facade: State<'_, Facade>) -> Result<(), String> {
    facade.supervisor().stop_all(ProjectId::from_raw(project));
    Ok(())
}

/// Restarts every currently-running process in a project (trusted only).
#[tauri::command]
pub async fn stack_restart_running(project: u64, facade: State<'_, Facade>) -> Result<(), String> {
    facade
        .supervisor()
        .restart_running(ProjectId::from_raw(project))
        .map_err(|err| err.to_string())
}

/// Writes typed text or raw control bytes to a running process's PTY.
#[tauri::command]
pub async fn pty_write(id: u64, data: String, facade: State<'_, Facade>) -> Result<(), String> {
    facade
        .supervisor()
        .write_stdin(ProcessId::from_raw(id), data.into_bytes())
        .await
        .map_err(|err| err.to_string())
}

/// Resizes a running process's PTY so the child sees the new dimensions (and SIGWINCH).
#[tauri::command]
pub async fn pty_resize(
    id: u64,
    cols: u16,
    rows: u16,
    facade: State<'_, Facade>,
) -> Result<(), String> {
    facade
        .supervisor()
        .resize(ProcessId::from_raw(id), cols, rows)
        .await
        .map_err(|err| err.to_string())
}

/// Attaches the terminal pane to a process: replays its raw scrollback as the first
/// channel message, then streams live PTY bytes. Cancels any previous attachment so a
/// single forwarder runs (the pane shows one process at a time).
#[tauri::command]
pub async fn pty_attach(
    id: u64,
    on_chunk: Channel<Vec<u8>>,
    facade: State<'_, Facade>,
    bridge: State<'_, PtyBridge>,
) -> Result<(), String> {
    let (scrollback, mut live) = facade
        .supervisor()
        .attach_pty(ProcessId::from_raw(id))
        .ok_or_else(|| "process has not started".to_string())?;
    // The scrollback is captured atomically with the live receiver, so sending it as the
    // first message preserves the core's no-gap/no-duplicate guarantee across IPC.
    on_chunk.send(scrollback).map_err(|err| err.to_string())?;
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            match live.recv().await {
                Ok(chunk) => {
                    if on_chunk.send(chunk.to_vec()).is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
    bridge.install(handle);
    Ok(())
}

/// Detaches the terminal pane — the pane closed or the selection moved away.
#[tauri::command]
pub async fn pty_detach(bridge: State<'_, PtyBridge>) -> Result<(), String> {
    bridge.clear();
    Ok(())
}
