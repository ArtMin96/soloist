//! A stand-in "lead" agent for the orchestration end-to-end walks.
//!
//! Launched by the app as an ordinary agent, it does what a real bound lead does over MCP — reach
//! the app's IPC socket and bind its session to the process it was launched in — but directly over
//! the same `soloist-ipc` wire the MCP server uses, so it embeds no MCP client. Binding is
//! authenticated by the connecting peer's process group: because the app spawns each agent into a
//! fresh group and this stub is that group's leader, its own connection binds to its own process,
//! exactly as `soloist-mcp` binds when an agent launches it.
//!
//! It then runs one of two arms, chosen by whether the spec dropped a coordination plan beside the
//! socket:
//!
//! * **Lineage** (no plan): `spawn_agent` a worker, then wait for a trigger file and close its own
//!   bound process — the one core removal that re-roots its workers, driven from outside the window.
//! * **Coordination** (plan present): seed the shared documents a bound agent produces — a
//!   scratchpad, a blocker chain of todos, and a comment stamped with this bound actor — then, on a
//!   trigger, re-write the scratchpad to bump its revision under the window's stale editor. This is
//!   how the panels walk forces a real revision conflict and a real comment author.
//!
//! Not product code: it lives under the e2e fixtures in its own workspace, built only by the e2e
//! harness, and links the real protocol crate so the wire format is single-sourced.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use soloist_core::{ProcessId, ScratchpadDoc, TodoDoc, TodoId, TodoStatus, PROCESS_ID_ENV};
use soloist_ipc::{read_frame, socket_path, write_frame, IpcRequest, IpcResponse, IpcResult};
use tokio::net::UnixStream;

/// The tool the lineage arm's worker is spawned from — supplied by the harness through the app's
/// environment. A real lead would decide this from its own task. Required in the lineage arm, never
/// defaulted: a lead that silently spawned nothing would fail the walk as a confusing timeout
/// instead of a named error.
const WORKER_TOOL_ENV: &str = "SOLOIST_E2E_WORKER_TOOL";

/// The trigger the lineage walk drops to make the lead close its own session process. Watched inside
/// the app's data directory (beside the IPC socket), which the walk resolves the same way — so no
/// extra environment is needed. One named const per side (the TS side names it in
/// `harness/leadAgent.ts`).
const CLOSE_SIGNAL_FILE: &str = "lead-close-signal";

/// Present in the data directory → the coordination arm; its JSON is the fixture data the panels
/// assert, single-sourced in the spec. One named const per side (TS: `harness/leadAgent.ts`).
const COORDINATION_PLAN_FILE: &str = "lead-coordination-plan";

/// The trigger the panels walk drops to make the lead re-write the scratchpad, bumping its revision
/// under the window's stale editor. One named const per side (TS: `harness/leadAgent.ts`).
const SCRATCHPAD_REWRITE_FILE: &str = "lead-scratchpad-rewrite";

/// How often a trigger file is polled — the lead is otherwise idle, so a relaxed poll suffices.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// The fixture data the coordination walk asserts. Single-sourced in the spec (TS) and handed to
/// the lead as JSON, so the values the panels check live in exactly one place; the lead fills the
/// rest of each disciplined document with fixed boilerplate the spec never inspects.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoordinationPlan {
    /// The scratchpad name the window opens and the lead re-writes.
    scratchpad: String,
    /// The scratchpad objective the lead first creates (the window opens this revision).
    objective_v1: String,
    /// The scratchpad objective the lead re-writes on the trigger (the concurrent edit that survives).
    objective_v2: String,
    /// The blocker todo's title — completing the blocked todo is refused until this one is done.
    blocker: String,
    /// The blocked todo's title — gated by the blocker.
    blocked: String,
    /// A todo carrying a comment, so the board shows the author the core stamps.
    commented: String,
    /// The comment body the lead writes on the commented todo.
    comment: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The process the app launched us in: the lead whose session we bind and, in the lineage arm,
    // close.
    let bound: u64 = std::env::var(PROCESS_ID_ENV)?.parse()?;
    let process = ProcessId::from_raw(bound);

    let socket = socket_path()?;
    // The plan and trigger files live beside the socket, in the app's data directory — resolved
    // identically by the app and this stub, so neither has to be told where the other put it.
    let data_dir = socket
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut stream = UnixStream::connect(&socket).await?;

    // Bind our session to the process the app launched us in — the way the MCP client does on
    // connect. Only a bound session records lineage and stamps a comment author, so this is what
    // makes both arms genuine.
    request(&mut stream, IpcRequest::BindSessionProcess { process }).await?;
    println!("lead bound to process {bound}");

    let plan_path = data_dir.join(COORDINATION_PLAN_FILE);
    if plan_path.exists() {
        let plan: CoordinationPlan = serde_json::from_slice(&std::fs::read(&plan_path)?)?;
        coordinate(&mut stream, &plan, &data_dir).await?;
    } else {
        lineage(&mut stream, process, &data_dir).await?;
    }

    // Nothing left to do but wait for the app to tear us down.
    std::future::pending::<()>().await;
    Ok(())
}

/// The lineage arm: spawn a worker under this bound lead, then close the lead on the walk's trigger
/// so the window can observe the worker re-root.
async fn lineage(
    stream: &mut UnixStream,
    process: ProcessId,
    data_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let worker_tool = std::env::var(WORKER_TOOL_ENV)
        .map_err(|_| format!("{WORKER_TOOL_ENV} is not set — the harness names the worker tool"))?;

    // Spawn the worker. It lands in our own project (the bound scope) and nests under us.
    request(
        stream,
        IpcRequest::SpawnAgent {
            tool: worker_tool.clone(),
            extra_args: Vec::new(),
        },
    )
    .await?;
    println!("lead spawned worker ({worker_tool})");

    // Wait for the harness to ask us to close, then remove our own process from the registry — the
    // one core action that re-roots our workers. The app reaps our process group as it closes us.
    let close_signal = data_dir.join(CLOSE_SIGNAL_FILE);
    while !close_signal.exists() {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    let _ = request(stream, IpcRequest::CloseProcess { process }).await;
    Ok(())
}

/// The coordination arm: seed the shared documents a bound agent produces, then re-write the
/// scratchpad on the walk's trigger to force a revision conflict under the window's stale editor.
async fn coordinate(
    stream: &mut UnixStream,
    plan: &CoordinationPlan,
    data_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create the scratchpad the window will edit, and remember the revision it is created at so the
    // re-write can guard on it (an update carries the revision it expects).
    let base_revision = scratchpad_revision(
        request(
            stream,
            IpcRequest::ScratchpadWrite {
                name: plan.scratchpad.clone(),
                doc: scratchpad_doc(&plan.objective_v1),
                expected_revision: None,
            },
        )
        .await?,
    )?;
    println!(
        "lead created scratchpad {} at revision {base_revision}",
        plan.scratchpad
    );

    // A blocker chain: `blocked` cannot be completed until `blocker` is done (the gate).
    let blocker = todo_id(
        request(
            stream,
            IpcRequest::TodoCreate {
                doc: todo_doc(&plan.blocker),
            },
        )
        .await?,
    )?;
    let blocked = todo_id(
        request(
            stream,
            IpcRequest::TodoCreate {
                doc: todo_doc(&plan.blocked),
            },
        )
        .await?,
    )?;
    request(
        stream,
        IpcRequest::TodoSetBlockers {
            todo: blocked,
            blockers: vec![blocker],
        },
    )
    .await?;

    // A todo carrying a comment — the core stamps the author from this bound session, so the board
    // shows who wrote it.
    let commented = todo_id(
        request(
            stream,
            IpcRequest::TodoCreate {
                doc: todo_doc(&plan.commented),
            },
        )
        .await?,
    )?;
    request(
        stream,
        IpcRequest::TodoCommentCreate {
            todo: commented,
            body: plan.comment.clone(),
        },
    )
    .await?;
    println!("lead seeded a blocker chain and a comment");

    // Wait for the walk's trigger, then re-write the scratchpad at the revision it was created at —
    // bumping it to the next revision under the window's stale editor, which forces the
    // optimistic-concurrency conflict when the window saves its own (now stale) edit.
    let rewrite = data_dir.join(SCRATCHPAD_REWRITE_FILE);
    while !rewrite.exists() {
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    request(
        stream,
        IpcRequest::ScratchpadWrite {
            name: plan.scratchpad.clone(),
            doc: scratchpad_doc(&plan.objective_v2),
            expected_revision: Some(base_revision),
        },
    )
    .await?;
    println!("lead re-wrote scratchpad {}", plan.scratchpad);
    Ok(())
}

/// A disciplined scratchpad document with the given objective and fixed valid boilerplate for the
/// rest — only the objective is asserted, but every required field must be present for the core to
/// accept the write.
fn scratchpad_doc(objective: &str) -> ScratchpadDoc {
    ScratchpadDoc {
        objective: objective.to_owned(),
        context: "Seeded by the e2e lead stub.".to_owned(),
        plan: vec!["Draft the change.".to_owned()],
        acceptance_criteria: vec!["The change is reviewed.".to_owned()],
        risks: vec!["none identified".to_owned()],
        status: "in progress".to_owned(),
        notes: None,
    }
}

/// A disciplined todo document with the given title and fixed valid boilerplate for the rest.
fn todo_doc(title: &str) -> TodoDoc {
    TodoDoc {
        title: title.to_owned(),
        description: "Seeded by the e2e lead stub.".to_owned(),
        acceptance_criteria: vec!["The work is reviewed.".to_owned()],
        risks: vec!["none identified".to_owned()],
        status: TodoStatus::Open,
    }
}

/// The revision a scratchpad write reply reports, or a typed error if the reply was some other shape.
fn scratchpad_revision(reply: IpcResponse) -> Result<u64, Box<dyn std::error::Error>> {
    match reply {
        IpcResponse::Scratchpad(view) => Ok(view.revision),
        other => Err(format!("expected a scratchpad reply, got {other:?}").into()),
    }
}

/// The id a todo reply reports, or a typed error if the reply was some other shape.
fn todo_id(reply: IpcResponse) -> Result<TodoId, Box<dyn std::error::Error>> {
    match reply {
        IpcResponse::Todo(view) => Ok(view.id),
        other => Err(format!("expected a todo reply, got {other:?}").into()),
    }
}

/// Sends one framed request and returns its reply, surfacing a typed app error or a transport
/// failure.
async fn request(
    stream: &mut UnixStream,
    req: IpcRequest,
) -> Result<IpcResponse, Box<dyn std::error::Error>> {
    write_frame(stream, &req).await?;
    match read_frame::<_, IpcResult>(stream).await? {
        Some(Ok(reply)) => Ok(reply),
        Some(Err(err)) => Err(format!("app refused {req:?}: {err}").into()),
        None => Err(format!("connection closed before replying to {req:?}").into()),
    }
}
