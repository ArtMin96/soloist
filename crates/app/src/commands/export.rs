//! The file-export command: the write half of the scratchpad "Export .md" flow.
//!
//! The frontend picks the destination with the native save dialog (`dialog:allow-save`) and passes
//! the chosen path here to persist the bytes. Writing already-projected read-model text to a
//! user-chosen file is I/O, not domain behaviour, so it lives in the adapter — the same way "Copy
//! Markdown" writes the clipboard — and never routes through the core façade. The path is the one
//! the user just consented to in the OS dialog; no scope is inferred here.

/// Writes `contents` (UTF-8 Markdown) to `path` — the file the save dialog returned. Surfaces any
/// I/O failure as a string the panel can show.
#[tauri::command]
pub async fn export_markdown(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|err| err.to_string())
}

/// Writes `bytes` to `path` — the file the save dialog returned. The diagram exports (rendered SVG,
/// raw `.mmd` source, rasterized PNG) are already-projected artifacts, so like [`export_markdown`]
/// this is adapter I/O to a path the user just consented to, never a route through the core façade.
#[tauri::command]
pub async fn export_bytes(path: String, bytes: Vec<u8>) -> Result<(), String> {
    std::fs::write(&path, bytes).map_err(|err| err.to_string())
}
