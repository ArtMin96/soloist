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
use tauri::ipc::{Channel, Response};
use tauri::State;
use tokio::sync::broadcast::error::RecvError;

use crate::pty_bridge::PtyBridge;

/// Runs a synchronous [`Facade`] operation on tokio's blocking pool and awaits it, so a
/// durable-store write's `fsync` (slow or full disk) can never park a runtime worker — no blocking
/// call runs on the runtime. The cloned `Arc` keeps the façade alive for the task; the closure
/// returns the command's own result. Commands whose bodies instead await the core (PTY writes,
/// process removal, agent detection) stay on the runtime and do not route through here.
pub(crate) async fn offload<T, F>(facade: &Arc<Facade>, op: F) -> T
where
    F: FnOnce(&Facade) -> T + Send + 'static,
    T: Send + 'static,
{
    let facade = Arc::clone(facade);
    tokio::task::spawn_blocking(move || op(&facade))
        .await
        .expect("a façade call must not panic on the blocking pool")
}

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
    offload(facade.inner(), |f| f.projects_snapshot())
        .await
        .map_err(|err| err.to_string())
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
    offload(facade.inner(), move |f| f.load_project(Path::new(&path)))
        .await
        .map_err(|err| err.to_string())
}

/// Removes a project from Soloist: closes its processes (live groups stopped and reaped
/// before anything is forgotten), deletes its durable record — the store cascades to its
/// project-scoped state (trust, todos, scratchpads, settings, …) — and announces
/// `ProjectRemoved`, which prompts the UI to re-read the project snapshot. Files on disk
/// are never touched. Routes to the one core removal the HTTP API and CLI also drive.
#[tauri::command]
pub async fn project_remove(project: u64, facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    facade
        .remove_project(ProjectId::from_raw(project))
        .await
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
    offload(facade.inner(), move |f| {
        f.trust_command(ProjectId::from_raw(project), &name)
    })
    .await
    .map_err(|err| err.to_string())
}

/// Every configured agent tool, for the launch picker to render instantly (no probing).
#[tauri::command]
pub async fn agent_list(facade: State<'_, Arc<Facade>>) -> Result<Vec<AgentTool>, String> {
    offload(facade.inner(), |f| f.agents().list_tools())
        .await
        .map_err(|err| err.to_string())
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
    offload(facade.inner(), move |f| {
        f.launch_agent(ProjectId::from_raw(project), &tool, extra_args)
    })
    .await
    .map(|id| id.get())
    .map_err(|err| err.to_string())
}

/// Starts one process; refused by the core trust gate if its command is untrusted.
#[tauri::command]
pub async fn proc_start(id: u64, facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    offload(facade.inner(), move |f| {
        f.supervisor().start(ProcessId::from_raw(id))
    })
    .await
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
    offload(facade.inner(), move |f| {
        f.supervisor().restart(ProcessId::from_raw(id))
    })
    .await
    .map_err(|err| err.to_string())
}

/// Resumes a stopped agent's last session ("Resume last session"): relaunches it with its
/// provider's resume command instead of starting fresh. Errors if the process has no last
/// session to resume (a command, terminal, or unsupported-provider agent).
#[tauri::command]
pub async fn agent_resume(id: u64, facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    offload(facade.inner(), move |f| {
        f.supervisor().resume(ProcessId::from_raw(id))
    })
    .await
    .map_err(|err| err.to_string())
}

/// Starts every trusted auto-start command in a project (untrusted ones are skipped).
#[tauri::command]
pub async fn stack_start(project: u64, facade: State<'_, Arc<Facade>>) -> Result<(), String> {
    offload(facade.inner(), move |f| {
        f.supervisor().start_all(ProjectId::from_raw(project))
    })
    .await
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
    offload(facade.inner(), move |f| {
        f.supervisor().restart_running(ProjectId::from_raw(project))
    })
    .await
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

/// The first byte of every PTY channel frame tags what follows: a live chunk the UI appends
/// ([`PTY_FRAME_CHUNK`]), or a raw-scrollback snapshot the UI must reset its emulator to
/// ([`PTY_FRAME_RESYNC`], sent first and again after the forwarder falls behind). Framing keeps
/// the efficient raw-bytes channel while letting the forwarder signal a re-sync — the UI mirror
/// of these values lives in `api.ts`.
const PTY_FRAME_CHUNK: u8 = 0;
const PTY_FRAME_RESYNC: u8 = 1;

/// Prefixes a payload with its frame tag and wraps it as a raw-bytes IPC response, so the channel
/// delivers a binary `ArrayBuffer` to the webview instead of a JSON number array — the full
/// scrollback replay crosses the boundary as bytes, with no per-byte JSON expansion.
fn pty_frame(tag: u8, bytes: &[u8]) -> Response {
    let mut framed = Vec::with_capacity(bytes.len() + 1);
    framed.push(tag);
    framed.extend_from_slice(bytes);
    Response::new(framed)
}

/// Attaches a terminal pane to a process: replays its raw scrollback as the first channel
/// message, then streams live PTY bytes. The keep-alive terminal pool runs several attachments
/// at once, each an independent forwarder. Returns the token that identifies this attachment;
/// [`pty_detach`] cancels it by that token.
#[tauri::command]
pub async fn pty_attach(
    id: u64,
    on_chunk: Channel<Response>,
    facade: State<'_, Arc<Facade>>,
    bridge: State<'_, PtyBridge>,
) -> Result<u64, String> {
    let pid = ProcessId::from_raw(id);
    let (scrollback, mut live) = facade
        .supervisor()
        .attach_pty(pid)
        .ok_or_else(|| "process has not started".to_string())?;
    // The scrollback is captured atomically with the live receiver, so sending it as the
    // first message preserves the core's no-gap/no-duplicate guarantee across IPC. It is a
    // resync frame: the emulator resets to it, the same way it recovers from a re-sync below.
    on_chunk
        .send(pty_frame(PTY_FRAME_RESYNC, &scrollback))
        .map_err(|err| err.to_string())?;
    let facade = Arc::clone(&facade);
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            match live.recv().await {
                Ok(chunk) => {
                    if on_chunk.send(pty_frame(PTY_FRAME_CHUNK, &chunk)).is_err() {
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => {
                    // The forwarder fell behind and the broadcast dropped chunks, leaving a
                    // hole in the byte stream that would desync the emulator mid-escape. Do
                    // not skip it: re-attach for a fresh, gap-free scrollback and push it as a
                    // resync so the UI resets to a coherent screen instead of rendering junk.
                    match facade.supervisor().attach_pty(pid) {
                        Some((scrollback, fresh)) => {
                            live = fresh;
                            if on_chunk
                                .send(pty_frame(PTY_FRAME_RESYNC, &scrollback))
                                .is_err()
                            {
                                break;
                            }
                        }
                        // The process is gone; nothing more to stream.
                        None => break,
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
    Ok(bridge.install(handle))
}

/// Detaches the attachment identified by `token` — the pane closed, switched away, or was
/// evicted from the keep-alive pool. Async commands execute out of invoke order, so a token that
/// has already been cleared is a no-op rather than touching a live forwarder.
#[tauri::command]
pub async fn pty_detach(token: u64, bridge: State<'_, PtyBridge>) -> Result<(), String> {
    bridge.clear(token);
    Ok(())
}

/// Resolves surfaced orphans the user chose to reap: SIGKILLs each listed process group
/// whose recorded identity still matches, and forgets its runtime-state record. "Leave
/// running" sends an empty list, so nothing is signalled — the dialog simply dismisses. A
/// group whose SIGKILL fails is reported so the UI can surface the error and keep its row.
#[tauri::command]
pub async fn orphans_resolve(
    pgids: Vec<i32>,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    let mut failures = Vec::new();
    for pgid in pgids {
        if let Err(err) = facade.supervisor().kill_orphan(pgid) {
            failures.push(format!("pgid {pgid} ({err})"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Could not stop leftover {}: {}",
            if failures.len() == 1 {
                "process"
            } else {
                "processes"
            },
            failures.join(", "),
        ))
    }
}
