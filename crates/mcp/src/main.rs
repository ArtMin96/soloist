//! `soloist-mcp`: the stdio MCP server. A thin, stateless adapter that forwards tool calls
//! to the running Soloist app over local IPC, scoped to one identity session.

mod args;
mod cache_hints;
mod client;
mod prompts;
mod server;
mod suggestions;
#[cfg(test)]
mod testing;
mod tools;

use std::sync::Arc;

use rmcp::transport::stdio;
use rmcp::ServiceExt;
use soloist_core::{resolve_reply_budget, ProcessId, PROCESS_ID_ENV};
use soloist_ipc::socket_path;

use client::AppClient;
use server::SoloistMcp;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bind to the process Soloist launched us in, if any, so our tool calls are attributed
    // to it. Absent or unparseable → an unbound session; `whoami` simply reports that.
    let bound = std::env::var(PROCESS_ID_ENV)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .map(ProcessId::from_raw);
    let client = Arc::new(AppClient::new(bound, socket_path()?));

    // Resolve which feature-tool groups to serve from the app's settings. If the app is
    // unreachable, fall back to the defaults (Key-Value off, the rest on) so the server still
    // starts and lists its core tools — a settings change is picked up on the next reconnect.
    let groups = client.mcp_tool_groups().await.unwrap_or_default();

    // Size every windowed reply to the ceiling the hosting agent enforces, resolved once: the
    // process we are bound to — and so the provider hosting us — cannot change for the life of a
    // stdio connection, and `tools/list` has to carry the figure before any call is made. An
    // unreachable app leaves the provider unknown, which resolves to the conservative default.
    let provider = client
        .whoami()
        .await
        .ok()
        .and_then(|whoami| whoami.provider);
    let budget = resolve_reply_budget(provider, |var| std::env::var(var).ok());

    // Serve over stdio until the MCP client disconnects. The connection to the app is opened
    // lazily on the first request, so this starts even when Soloist is not running.
    let service = SoloistMcp::new(client, groups, budget)
        .serve(stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
