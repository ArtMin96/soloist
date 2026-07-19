//! The JSON codec for the todo columns that persist domain types verbatim.
//!
//! The document, tag set, blocker ids, and comments are stored as their own JSON, so the persisted
//! shapes are exactly the domain types and cannot drift from them.

use soloist_core::{Comment, StoreError, TodoDoc, TodoId};

/// Serializes a [`TodoDoc`] to the JSON the `doc` column stores.
pub(crate) fn serialize_doc(doc: &TodoDoc) -> Result<String, StoreError> {
    serde_json::to_string(doc).map_err(|err| StoreError::Backend(format!("serialize todo: {err}")))
}

/// Deserializes the `doc` column's JSON into a [`TodoDoc`].
pub(crate) fn decode_doc(json: &str) -> Result<TodoDoc, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize todo: {err}")))
}

/// Serializes a string list (tags) to the JSON array its column stores.
pub(crate) fn serialize_strings(items: &[String]) -> Result<String, StoreError> {
    serde_json::to_string(items)
        .map_err(|err| StoreError::Backend(format!("serialize todo tags: {err}")))
}

/// Deserializes a JSON string array (tags).
pub(crate) fn decode_strings(json: &str) -> Result<Vec<String>, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize todo tags: {err}")))
}

/// Serializes the blocker ids to a JSON array of raw ids.
pub(crate) fn serialize_blockers(blockers: &[TodoId]) -> Result<String, StoreError> {
    let raw: Vec<u64> = blockers.iter().map(|id| id.get()).collect();
    serde_json::to_string(&raw)
        .map_err(|err| StoreError::Backend(format!("serialize todo blockers: {err}")))
}

/// Deserializes a JSON array of raw ids into blocker ids.
pub(crate) fn decode_blockers(json: &str) -> Result<Vec<TodoId>, StoreError> {
    let raw: Vec<u64> = serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize todo blockers: {err}")))?;
    Ok(raw.into_iter().map(TodoId::from_raw).collect())
}

/// Serializes the comment list to the JSON array its column stores.
pub(crate) fn serialize_comments(comments: &[Comment]) -> Result<String, StoreError> {
    serde_json::to_string(comments)
        .map_err(|err| StoreError::Backend(format!("serialize todo comments: {err}")))
}

/// Deserializes a JSON array of comments.
pub(crate) fn decode_comments(json: &str) -> Result<Vec<Comment>, StoreError> {
    serde_json::from_str(json)
        .map_err(|err| StoreError::Backend(format!("deserialize todo comments: {err}")))
}
