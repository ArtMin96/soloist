//! Coordination lease tools: acquiring, checking, and releasing project-scoped lease locks.
//!
//! A lease is owned by the caller's bound process and auto-releases when that process closes or
//! the lease's TTL expires — both enforced in the core, not here. "Signals, not ownership":
//! acquiring is non-blocking, and a key another process holds is reported rather than waited on.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{LockAcquireArg, LockKeyArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

/// The lease lifetime used when a caller does not specify one — long enough for a typical
/// coordinated step, short enough that a holder which crashed without releasing frees the key
/// soon after.
const DEFAULT_LEASE_TTL_MS: u64 = 5 * 60 * 1000;

#[tool_router(router = lock_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Acquire a project-scoped lease lock by key, owned by your bound process and auto-released when it closes or the lease expires. Non-blocking: if another process holds the key, returns that holder instead of waiting (outcome \"held\"). Re-acquire the same key to renew. ttl_ms defaults if omitted."
    )]
    pub(crate) async fn lock_acquire(
        &self,
        Parameters(LockAcquireArg { key, ttl_ms }): Parameters<LockAcquireArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let request = IpcRequest::LockAcquire {
            key,
            ttl_ms: ttl_ms.unwrap_or(DEFAULT_LEASE_TTL_MS),
        };
        match self.client.request(request).await {
            Ok(IpcResponse::LeaseOutcome(outcome)) => structured(&outcome),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Get the current holder of a project-scoped lease by key. Returns the holder, or null under `holder` if the key is free or its lease has expired."
    )]
    pub(crate) async fn lock_status(
        &self,
        Parameters(LockKeyArg { key }): Parameters<LockKeyArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::LockStatus { key }).await {
            Ok(IpcResponse::LeaseStatus(holder)) => {
                structured(&serde_json::json!({ "holder": holder }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Release a project-scoped lease by key if your bound process holds it. Returns whether your lease was released under `released`; you cannot release a lease another process holds."
    )]
    pub(crate) async fn lock_release(
        &self,
        Parameters(LockKeyArg { key }): Parameters<LockKeyArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::LockRelease { key }).await {
            Ok(IpcResponse::LeaseReleased(released)) => {
                structured(&serde_json::json!({ "released": released }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
