//! Where Soloist keeps its per-user files, and the IPC socket within it.
//!
//! Resolved the same way for every binary so the app (server) and the MCP server
//! (client) agree on the socket path without the app passing it — the MCP server is
//! launched by a third-party MCP host, not by the app. This is the single source for
//! the data-directory convention and for creating it: both the SQLite store and the
//! IPC server obtain the directory through [`ensure_data_dir`], so it is created — and
//! restricted to its owner — in exactly one place.

use std::io;
use std::path::PathBuf;

/// The Unix-socket file the app listens on and the MCP server connects to.
const SOCKET_FILE: &str = "soloist-ipc.sock";

/// The environment variable that overrides the data directory — the single source of its
/// name. A generated MCP client snippet must carry it when it is set (the helper is
/// launched by the MCP host with a fresh environment, so without it the helper would
/// resolve a different directory and miss the socket).
pub const DATA_DIR_ENV: &str = "SOLOIST_APP_DATA_DIR";

/// Owner-only permissions (`rwx------`) for the data directory. The IPC socket lives
/// inside it, and connecting to a Unix socket requires search (`x`) on every path
/// component — so denying other local users access to the directory is what keeps them
/// off the socket (and away from the SQLite database beside it).
#[cfg(unix)]
const DATA_DIR_MODE: u32 = 0o700;

/// Why the data directory could not be resolved: no usable location is configured.
#[derive(Debug, thiserror::Error)]
#[error("cannot resolve the data directory: set {DATA_DIR_ENV} or HOME")]
pub struct DataDirError;

/// Soloist's per-user data directory: `$SOLOIST_APP_DATA_DIR`, else
/// `$XDG_DATA_HOME/soloist`, else `$HOME/.local/share/soloist`.
pub fn data_dir() -> Result<PathBuf, DataDirError> {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
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

/// Whether the data directory is overridden via [`DATA_DIR_ENV`] — the signal that a
/// generated client snippet must carry the variable for the helper to find the socket.
pub fn data_dir_overridden() -> bool {
    std::env::var_os(DATA_DIR_ENV).is_some()
}

/// The IPC socket path inside the data directory.
pub fn socket_path() -> Result<PathBuf, DataDirError> {
    Ok(data_dir()?.join(SOCKET_FILE))
}

/// Resolves the data directory, creates it if missing, and restricts it to its owner,
/// returning its path. The single place the data directory is created — both the store
/// (for the database) and the IPC server (for the socket) go through here, so the
/// owner-only restriction always holds and the directory is never created twice with
/// different permissions.
pub fn ensure_data_dir() -> io::Result<PathBuf> {
    let dir = data_dir().map_err(|err| io::Error::new(io::ErrorKind::NotFound, err.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(DATA_DIR_MODE))?;
    }
    Ok(dir)
}

/// The IPC socket path, with its (owner-only) data directory ensured to exist — what the
/// app's IPC server binds. The client uses [`socket_path`] instead, since only the server
/// creates the directory.
pub fn ensure_socket_path() -> io::Result<PathBuf> {
    Ok(ensure_data_dir()?.join(SOCKET_FILE))
}
