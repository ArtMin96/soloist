//! Coordination kv tools: the project-scoped JSON key-value store agents use for small shared state.
//!
//! Four tools cover the full surface: `kv_set` (create or replace), `kv_get` (read or `null`),
//! `kv_delete` (remove), and `kv_list` (all pairs ordered by key). Values are any JSON — objects,
//! arrays, strings, numbers, booleans. The store is project-scoped and durable; entries survive an
//! app restart. Scope is resolved in the core, not here.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData};
use rmcp::{tool, tool_router};
use soloist_ipc::{IpcRequest, IpcResponse};

use crate::args::{KvKeyArg, KvSetArg};
use crate::server::SoloistMcp;
use crate::tools::reply::{app_error, structured, unexpected};

#[tool_router(router = kv_router, vis = "pub(crate)")]
impl SoloistMcp {
    #[tool(
        description = "Store a JSON value at key in the project's shared key-value store. Creates or replaces the entry. The value can be any JSON — object, array, string, number, or boolean. Prefer structured objects over raw strings so the data stays queryable."
    )]
    pub(crate) async fn kv_set(
        &self,
        Parameters(KvSetArg { key, value }): Parameters<KvSetArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::KvSet { key, value }).await {
            Ok(IpcResponse::KvValue(_)) => structured(&serde_json::json!({ "ok": true })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Read the JSON value at key from the project's shared key-value store. Returns the value, or null if the key does not exist."
    )]
    pub(crate) async fn kv_get(
        &self,
        Parameters(KvKeyArg { key }): Parameters<KvKeyArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::KvGet { key }).await {
            Ok(IpcResponse::KvValue(value)) => structured(&serde_json::json!({ "value": value })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "Delete the entry at key from the project's shared key-value store. Returns true if an entry was removed, false if the key did not exist."
    )]
    pub(crate) async fn kv_delete(
        &self,
        Parameters(KvKeyArg { key }): Parameters<KvKeyArg>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::KvDelete { key }).await {
            Ok(IpcResponse::KvDeleted(removed)) => {
                structured(&serde_json::json!({ "removed": removed }))
            }
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }

    #[tool(
        description = "List all key-value pairs in the project's shared key-value store, ordered by key. Use this to inspect the full shared state before deciding what to write."
    )]
    pub(crate) async fn kv_list(&self) -> Result<CallToolResult, ErrorData> {
        match self.client.request(IpcRequest::KvList).await {
            Ok(IpcResponse::KvPairs(pairs)) => structured(&serde_json::json!({ "entries": pairs })),
            Ok(_) => Err(unexpected()),
            Err(err) => app_error(&err),
        }
    }
}
