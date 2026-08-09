//! Shared helpers that turn an IPC reply into the MCP wire result, used by every tool
//! category. Single-sourced here so the structured-result shape and the error model stay
//! identical across categories.

use rmcp::model::{CallToolResult, ErrorData};
use serde::Serialize;
use soloist_ipc::IpcError;

use crate::client::ClientError;

/// The field a refusal's own words are carried under, beside the typed discriminator the app
/// decided. One name, so a caller reads every refusal the same way.
const MESSAGE: &str = "message";

/// Serializes a reply into a structured tool result. The value must serialize to a JSON
/// **object** — the MCP spec constrains `structuredContent` to an object, and clients refuse
/// a bare array — so a list reply is always wrapped in a keyed object first.
pub(crate) fn structured<T: Serialize>(value: &T) -> Result<CallToolResult, ErrorData> {
    serde_json::to_value(value)
        .map(|value| {
            debug_assert!(
                value.is_object(),
                "structuredContent must be a JSON object, got: {value}"
            );
            CallToolResult::structured(value)
        })
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
        ClientError::App(app) if app.is_request_error() => Ok(refusal(app)),
        _ => Err(ErrorData::internal_error(err.to_string(), None)),
    }
}

/// A refusal the caller can act on, carried as **data** as well as prose: the discriminator the
/// core decided, whatever that refusal carries with it, and the sentence it reads as.
///
/// The protocol offers exactly two ways to fail a tool call and no field for saying which failure
/// it was, so a caller that has to tell one refusal from another — an operation somebody stopped
/// from one that failed, a project it should ask the user to trust from a credential nobody
/// arranged — would otherwise have to read the wording. It matches on the wire error's own
/// discriminator instead. The sentence stays in `content` too, because a tool result's text is what
/// a model is shown and not every client reads structured data.
///
/// One rule shapes the body: the wire error's own fields, with a **structured detail's** fields
/// lifted beside the discriminator rather than nested under it. A detail is the only object-valued
/// field the wire error has, and its fields are what a caller acts on — the version-control reason,
/// the todos still blocking one, the cap a payload exceeded — so they read better one level up.
fn refusal(err: &IpcError) -> CallToolResult {
    let mut body = serde_json::Map::new();
    // Adjacently tagged, so it always serializes to an object carrying its own discriminator;
    // anything else would be a serde change rather than a runtime case.
    if let Ok(serde_json::Value::Object(wire)) = serde_json::to_value(err) {
        for (field, value) in wire {
            match value {
                serde_json::Value::Object(detail) => body.extend(detail),
                value => {
                    body.insert(field, value);
                }
            }
        }
    }
    body.insert(MESSAGE.to_owned(), serde_json::json!(err.to_string()));
    CallToolResult::structured_error(serde_json::Value::Object(body))
}

/// The app returned a response of the wrong shape — a protocol mismatch, not a user error.
pub(crate) fn unexpected() -> ErrorData {
    ErrorData::internal_error("the app returned an unexpected response".to_string(), None)
}
