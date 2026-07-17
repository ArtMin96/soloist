//! A stand-in "lead" agent for the orchestration end-to-end walk.
//!
//! Launched by the app as an ordinary agent, it does what a real bound lead does over MCP — reach
//! the app's IPC socket, bind its session to the process it was launched in, and `spawn_agent` a
//! worker — but directly over the same `soloist-ipc` wire the MCP server uses, so it embeds no MCP
//! client. Binding is authenticated by the connecting peer's process group: because the app spawns
//! each agent into a fresh group and this stub is that group's leader, its own connection binds to
//! its own process, exactly as `soloist-mcp` binds when an agent launches it.
//!
//! Then it waits for a trigger file and closes its own bound process — the one core removal that
//! re-roots its workers, driven from outside the window like a real MCP caller.
//!
//! Not product code: it lives under the e2e fixtures in its own workspace, built only by the e2e
//! harness, and links the real protocol crate so the wire format is single-sourced.

use std::path::PathBuf;
use std::time::Duration;

use soloist_core::{ProcessId, PROCESS_ID_ENV};
use soloist_ipc::{read_frame, socket_path, write_frame, IpcRequest, IpcResult};
use tokio::net::UnixStream;

/// The tool the worker is spawned from — supplied by the harness through the app's environment.
/// A real lead would decide this from its own task. Required, never defaulted: a lead that
/// silently spawned nothing would fail the walk as a confusing timeout instead of a named error.
const WORKER_TOOL_ENV: &str = "SOLOIST_E2E_WORKER_TOOL";

/// The trigger the walk drops to make the lead close its own session process. Watched inside the
/// app's data directory (beside the IPC socket), which the walk resolves the same way — so no extra
/// environment is needed. One named const per side of the boundary (the TS side names it in
/// `harness/leadAgent.ts`).
const CLOSE_SIGNAL_FILE: &str = "lead-close-signal";
/// How often the trigger file is polled — the lead is otherwise idle, so a relaxed poll suffices.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The process the app launched us in: the lead whose session we bind and, at the end, close.
    let bound: u64 = std::env::var(PROCESS_ID_ENV)?.parse()?;
    let process = ProcessId::from_raw(bound);

    let worker_tool = std::env::var(WORKER_TOOL_ENV)
        .map_err(|_| format!("{WORKER_TOOL_ENV} is not set — the harness names the worker tool"))?;

    let socket = socket_path()?;
    // The trigger file lives beside the socket, in the app's data directory — resolved identically
    // by the app and this stub, so neither has to be told where the other put it.
    let close_signal = socket
        .parent()
        .map(|dir| dir.join(CLOSE_SIGNAL_FILE))
        .unwrap_or_else(|| PathBuf::from(CLOSE_SIGNAL_FILE));

    let mut stream = UnixStream::connect(&socket).await?;

    // Bind our session to the process the app launched us in — the way the MCP client does on
    // connect. Only a bound session records lineage, so this is what makes a spawned worker nest.
    exchange(&mut stream, IpcRequest::BindSessionProcess { process }).await?;
    println!("lead bound to process {bound}");

    // Spawn the worker. It lands in our own project (the bound scope) and nests under us.
    exchange(
        &mut stream,
        IpcRequest::SpawnAgent {
            tool: worker_tool.clone(),
            extra_args: Vec::new(),
        },
    )
    .await?;
    println!("lead spawned worker ({worker_tool})");

    // Wait for the harness to ask us to close, then remove our own process from the registry — the
    // one core action that re-roots our workers. The app reaps our process group as it closes us,
    // so the wait for a reply below simply ends when we are killed.
    while !close_signal.exists() {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    let _ = exchange(&mut stream, IpcRequest::CloseProcess { process }).await;

    // Nothing left to do but wait for the app to tear us down.
    std::future::pending::<()>().await;
    Ok(())
}

/// Sends one framed request and reads its reply, surfacing a typed app error or a transport failure.
/// The reply for a bind/spawn/close is an ack or the new id; we only need that it succeeded.
async fn exchange(
    stream: &mut UnixStream,
    request: IpcRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    write_frame(stream, &request).await?;
    match read_frame::<_, IpcResult>(stream).await? {
        Some(Ok(_)) => Ok(()),
        Some(Err(err)) => Err(format!("app refused {request:?}: {err}").into()),
        None => Err(format!("connection closed before replying to {request:?}").into()),
    }
}
