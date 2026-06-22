//! Where Soloist keeps its per-user files, and the IPC socket within it.
//!
//! Resolved the same way for every binary so the app (server) and the MCP server
//! (client) agree on the socket path without the app passing it — the MCP server is
//! launched by a third-party MCP host, not by the app. This is the single source for
//! the data-directory convention; the store delegates to it.

use std::path::PathBuf;

/// The Unix-socket file the app listens on and the MCP server connects to.
const SOCKET_FILE: &str = "soloist-ipc.sock";

/// Why the data directory could not be resolved: no usable location is configured.
#[derive(Debug, thiserror::Error)]
#[error("cannot resolve the data directory: set SOLOIST_APP_DATA_DIR or HOME")]
pub struct DataDirError;

/// Soloist's per-user data directory: `$SOLOIST_APP_DATA_DIR`, else
/// `$XDG_DATA_HOME/soloist`, else `$HOME/.local/share/soloist`.
pub fn data_dir() -> Result<PathBuf, DataDirError> {
    if let Some(dir) = std::env::var_os("SOLOIST_APP_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("soloist"));
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".local/share/soloist"));
    }
    Err(DataDirError)
}

/// The IPC socket path inside the data directory.
pub fn socket_path() -> Result<PathBuf, DataDirError> {
    Ok(data_dir()?.join(SOCKET_FILE))
}
