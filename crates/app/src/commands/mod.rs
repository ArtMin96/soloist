//! The Tauri command surface: thin wrappers that route to the one core behaviour.
//!
//! No business logic lives here. Each command translates webview arguments into a call
//! on the [`Facade`]/supervisor and maps the typed core error to a string the UI can
//! render — so "restart", "start all", and the trust gate are implemented exactly once,
//! in the core. PTY bytes stream over a [`Channel`] (the high-throughput IPC primitive);
//! status and config changes ride the domain-event bus from `lib.rs`.
//!
//! [`Channel`]: tauri::ipc::Channel
//!
//! The durable-settings commands live in the [`settings`] submodule and are re-exported, so
//! the whole command surface stays under one `commands::` namespace in the invoke handler.

mod coordination;
mod orchestration;
mod project_settings;
mod settings;
mod timers;
pub use coordination::*;
pub use orchestration::*;
pub use project_settings::*;
pub use settings::*;
pub use timers::*;

use std::path::Path;
use std::sync::Arc;

use soloist_core::{
    AgentTool, DetectedTool, Facade, ProcessId, ProcessView, ProjectId, ProjectLoad, ProjectView,
};
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::broadcast::error::RecvError;

use crate::pty_bridge::PtyBridge;

/// The current process read model — the snapshot half of snapshot-then-deltas.
#[tauri::command]
pub async fn proc_list(facade: State<'_, Arc<Facade>>) -> Result<Vec<ProcessView>, String> {
    Ok(facade.snapshot())
}

/// The project read model — every opened project's display identity, name and icon already
/// resolved by the core. The snapshot half of snapshot-then-deltas for projects; a live open
/// arrives as a `ProjectOpened` event that prompts the UI to re-read this.
#[tauri::command]
pub async fn project_list(facade: State<'_, Arc<Facade>>) -> Result<Vec<ProjectView>, String> {
    facade.projects_snapshot().map_err(|err| err.to_string())
}

/// Loads a project from a folder path: auto-creates a `solo.yml` from detected commands
/// when the folder has none, registers each command (trust-gated), reconciles leftover
/// process groups, and starts the trusted auto-start subset. The registration and status
/// events repopulate the read model; the returned [`ProjectLoad`] carries how many
/// processes were declared and whether the `solo.yml` was just created, so the UI can
/// confirm what happened instead of leaving the screen unchanged.
#[tauri::command]
pub async fn project_load(
    path: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<ProjectLoad, String> {
    facade
        .load_project(Path::new(&path))
        .map_err(|err| err.to_string())
}

/// Trusts a project's command by name so it can start (A6). Routes to the one core
/// trust gate; the read model clears the command's blocked state, which the UI re-reads.
#[tauri::command]
pub async fn config_trust(
    project: u64,
    name: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    facade
        .trust_command(ProjectId::from_raw(project), &name)
        .map_err(|err| err.to_string())
}

/// Every configured agent tool, for the launch picker to render instantly (no probing).
#[tauri::command]
pub async fn agent_list(facade: State<'_, Arc<Facade>>) -> Result<Vec<AgentTool>, String> {
    facade.agents().list_tools().map_err(|err| err.to_string())
}

/// Each configured agent tool paired with whether its CLI appears installed, by probing
/// `<command> --version` off the runtime. The picker badges installed tools; this is slower
/// than [`agent_list`], so the UI lists first and fills in detection when this resolves.
#[tauri::command]
pub async fn agent_detect(facade: State<'_, Arc<Facade>>) -> Result<Vec<DetectedTool>, String> {
    facade
        .agents()
        .detect_installed()
        .await
        .map_err(|err| err.to_string())
}

/// Launches an agent tool as an interactive Agent process in a project and starts it,
/// returning its process id. `extra_args` are appended for this one launch ("agent with
/// flags"). Routes to the one core launch behaviour, which runs the agent on a real PTY and
/// passes the environment through so the CLI's own native login works.
#[tauri::command]
pub async fn agent_launch(
    project: u64,
    tool: String,
    extra_args: Vec<String>,
    facade: State<'_, Arc<Facade>>,
) -> Result<u64, String> {
    facade
        .launch_agent(ProjectId::from_raw(project), &tool, extra_args)
        .map(|id| id.get())
        .map_err(|err| err.to_string())
}

/// Starts one process; refused by the core trust gate if its command is untrusted.
#[tauri::command]
pub async fn proc_start(id: u64, facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    facade
        .supervisor()
        .start(ProcessId::from_raw(id))
        .map_err(|err| err.to_string())
}

/// Requests a graceful stop of one process; reports whether it was found live.
#[tauri::command]
pub async fn proc_stop(id: u64, facade: State<'_, Arc<Facade>>) -> Result<bool, String> {
    Ok(facade.supervisor().stop(ProcessId::from_raw(id)))
}

/// Restarts one process (stop then start with its saved config); trust-gated.
#[tauri::command]
pub async fn proc_restart(id: u64, facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    facade
        .supervisor()
        .restart(ProcessId::from_raw(id))
        .map_err(|err| err.to_string())
}

/// Starts every trusted auto-start command in a project (untrusted ones are skipped).
#[tauri::command]
pub async fn stack_start(project: u64, facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    facade
        .supervisor()
        .start_all(ProjectId::from_raw(project))
        .map(|_summary| ())
        .map_err(|err| err.to_string())
}

/// Stops every live process in a project.
#[tauri::command]
pub async fn stack_stop(project: u64, facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    facade.supervisor().stop_all(ProjectId::from_raw(project));
    Ok(())
}

/// Restarts every currently-running process in a project (trusted only).
#[tauri::command]
pub async fn stack_restart_running(
    project: u64,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    facade
        .supervisor()
        .restart_running(ProjectId::from_raw(project))
        .map_err(|err| err.to_string())
}

/// Writes typed text or raw control bytes to a running process's PTY.
#[tauri::command]
pub async fn pty_write(
    id: u64,
    data: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
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
    facade: State<'_, Arc<Facade>>,
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
    facade: State<'_, Arc<Facade>>,
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

/// Resolves surfaced orphans the user chose to reap: SIGKILLs each listed process
/// group and forgets its runtime-state record. "Leave running" sends an empty list, so
/// nothing is signalled — the dialog simply dismisses.
#[tauri::command]
pub async fn orphans_resolve(
    pgids: Vec<i32>,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    for pgid in pgids {
        facade.supervisor().kill_orphan(pgid);
    }
    Ok(())
}
