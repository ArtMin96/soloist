//! Shared helpers that turn an IPC reply into the MCP wire result, used by every tool
//! category. Single-sourced here so the structured-result shape and the error model stay
//! identical across categories.

use rmcp::model::{CallToolResult, Content, ErrorData};
use serde::Serialize;

use crate::client::ClientError;

/// Serializes a reply into a structured tool result.
pub(crate) fn structured<T: Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    serde_json::to_value(value)
        .map(CallToolResult::structured)
        .map_err(|err| ErrorData::internal_error(err.to_string(), None))
}

/// A structured acknowledgement for a state-setting tool (register / select).
pub(crate) fn acked() -> Result<CallToolResult, ErrorData> {
    structured(&serde_json::json!({ "ok": true }))
}

/// Maps a failed request to the agent-visible failure, per the MCP error model. A
/// request-caused refusal (untrusted, out of scope, no project selected, unknown
/// process/project/tool) becomes a tool-execution error (`isError: true`) — actionable
/// feedback the model can self-correct on. A transport or server failure (app down, timeout,
/// internal) stays a protocol error, which the model is less likely to recover from.
pub(crate) fn app_error(err: &ClientError) -> Result<CallToolResult, ErrorData> {
    match err {
        ClientError::App(app) if app.is_request_error() => {
            Ok(CallToolResult::error(vec![Content::text(app.to_string())]))
        }
        _ => Err(ErrorData::internal_error(err.to_string(), None)),
    }
}

/// The app returned a response of the wrong shape — a protocol mismatch, not a user error.
pub(crate) fn unexpected() -> ErrorData {
    ErrorData::internal_error("the app returned an unexpected response".to_string(), None)
}
