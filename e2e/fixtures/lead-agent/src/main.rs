//! A bound lead-agent stand-in for the real-window orchestration journeys.
//!
//! The fixture talks directly to the same authenticated IPC surface as `soloist-mcp`. Scenario
//! modules own the behavior for mailbox, timer, coordination, and lineage walks; this entry point
//! only binds the launched process, selects the requested scenario, and keeps the fixture alive so
//! observable `Running` state cannot race a completed scenario process.

mod coordination;
mod ipc;
mod lineage;
mod mailbox;
mod plans;
mod timers;

use std::io::Write;
use std::path::{Path, PathBuf};

use soloist_core::{ProcessId, PROCESS_ID_ENV};
use soloist_ipc::{socket_path, IpcRequest};
use tokio::net::UnixStream;

use ipc::{request, FixtureResult};
use plans::{CoordinationPlan, MailboxPlan, MailboxRole, TimerPlan};

/// The OSC title the lead sets once and then leaves alone.
///
/// The lead stands in for a provider read by the OSC-title idle heuristic, and an agent whose
/// provider signal never appears is never classified at all — not idle, not anything. A title-less
/// lead can therefore never be woken by anything gated on idle, which is every addressed message
/// the app queues for it. So the fixture emits the one signal its provider is read from, which is
/// also what makes its idle state deterministic: it settles idle a few samples in and stays there.
/// Whether the real CLI reliably sets a title is not something this repo records — the heuristic
/// assumes it, and the fixture mimics the heuristic's assumption rather than an observed CLI.
const LEAD_TITLE: &str = "soloist e2e lead";

#[tokio::main(flavor = "current_thread")]
async fn main() -> FixtureResult<()> {
    let bound: u64 = std::env::var(PROCESS_ID_ENV)?.parse()?;
    let process = ProcessId::from_raw(bound);
    let socket = socket_path()?;
    let data_dir = socket
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut stream = UnixStream::connect(&socket).await?;

    request(&mut stream, IpcRequest::BindSessionProcess { process }).await?;
    println!("lead bound to process {bound}");

    let mailbox_path = data_dir.join(plans::MAILBOX_PLAN_FILE);
    let timer_path = data_dir.join(plans::TIMER_PLAN_FILE);
    let coordination_path = data_dir.join(plans::COORDINATION_PLAN_FILE);
    let role = MailboxRole::from_args();
    if role.is_none() {
        announce_title()?;
    }
    if let Some(role) = role {
        let plan: MailboxPlan = serde_json::from_slice(&std::fs::read(mailbox_path)?)?;
        mailbox::worker(&mut stream, role, &plan).await?;
    } else if mailbox_path.exists() {
        let plan: MailboxPlan = serde_json::from_slice(&std::fs::read(mailbox_path)?)?;
        mailbox::lead(&mut stream, &plan).await?;
    } else if timer_path.exists() {
        let plan: TimerPlan = serde_json::from_slice(&std::fs::read(timer_path)?)?;
        timers::run(&mut stream, &plan).await?;
    } else if coordination_path.exists() {
        let plan: CoordinationPlan = serde_json::from_slice(&std::fs::read(coordination_path)?)?;
        coordination::run(&mut stream, &plan, &data_dir).await?;
    } else {
        lineage::run(&mut stream, process, &data_dir).await?;
    }

    std::future::pending::<()>().await;
    Ok(())
}

fn announce_title() -> FixtureResult<()> {
    print!("\u{1b}]0;{LEAD_TITLE}\u{7}");
    std::io::stdout().flush()?;
    Ok(())
}
