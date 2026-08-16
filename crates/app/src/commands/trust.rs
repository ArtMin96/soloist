//! The local user's trust surface: the requests awaiting a decision, deciding them, and reviewing
//! or taking back what is already trusted.
//!
//! No logic here. Which variant an approval authorizes, whether it still matches what was
//! displayed, and what a grant records are all decided in the core — these commands exist because
//! the *person* is the only one allowed to make the decision, and the desktop UI is where the
//! person is. Nothing on this surface is reachable over MCP or the loopback API.

use std::sync::Arc;

use soloist_core::{Facade, ProjectId, TrustGrant, TrustRequest, TrustRequestId};
use tauri::State;

/// Every trust request in a project still awaiting the user's decision — the read the approval
/// dialog opens from, and re-reads after a `TrustRequested` or `TrustRequestResolved` event.
#[tauri::command]
pub async fn trust_requests(
    project: u64,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<TrustRequest>, String> {
    Ok(facade
        .blocking(move |f| f.pending_trust_requests(ProjectId::from_raw(project)))
        .await)
}

/// Approves a request, trusting exactly the variant the dialog displayed. `variant_hash` is the
/// key that was on screen; the core refuses the grant unless it still matches the request's own
/// pinned spec, so a stale dialog can never authorize a command the user did not read.
#[tauri::command]
pub async fn trust_request_approve(
    request: u64,
    variant_hash: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    facade
        .blocking(move |f| {
            f.approve_trust_request(TrustRequestId::from_raw(request), &variant_hash)
        })
        .await
        .map_err(|err| err.to_string())
}

/// Declines a request. Nothing is trusted; the requester is told.
#[tauri::command]
pub async fn trust_request_deny(
    request: u64,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.deny_trust_request(TrustRequestId::from_raw(request)))
        .await
        .map_err(|err| err.to_string())
}

/// Every command variant trusted in a project, with the provenance of each — what the review list
/// renders so a grant made at an agent's asking is tellable from one the user authored.
#[tauri::command]
pub async fn trust_grants(
    project: u64,
    facade: State<'_, Arc<Facade>>,
) -> Result<Vec<TrustGrant>, String> {
    facade
        .blocking(move |f| f.list_trusted_commands(ProjectId::from_raw(project)))
        .await
        .map_err(|err| err.to_string())
}

/// Takes back a grant. The core re-checks trust on every start, so the variant is refused again
/// the next time anything tries to run it.
#[tauri::command]
pub async fn trust_revoke(
    project: u64,
    variant_hash: String,
    facade: State<'_, Arc<Facade>>,
) -> Result<(), String> {
    facade
        .blocking(move |f| f.revoke_command_trust(ProjectId::from_raw(project), &variant_hash))
        .await
        .map_err(|err| err.to_string())
}
