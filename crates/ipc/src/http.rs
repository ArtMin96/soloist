//! The local HTTP API's shared contract: the loopback port, the per-launch auth token and
//! its header, and the runtime file recording both after the server binds.
//!
//! Defined here, in the transport crate, so the two halves agree without one telling the
//! other: the in-app server (`soloist-httpapi`) mints a fresh token, writes it beside the
//! bound port in the (owner-only) runtime file, and requires it on every request; the
//! `soloist` CLI reads the file — reachable only within the owning user's `0700` data
//! directory — and sends the token back. The header name is the one definition the server
//! checks and the client sends; the value is a secret, per launch.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::{data_dir, ensure_data_dir, DataDirError};

/// The loopback port the HTTP API prefers. The server falls back to a nearby port if it
/// is taken and records the chosen one in the [runtime file](runtime_file_path).
pub const DEFAULT_PORT: u16 = 24678;

/// The header every request must carry, holding the running server's [per-launch
/// token](generate_token). The loopback bind and localhost CORS keep remote and
/// cross-origin callers out, but the port is TCP and any local user can reach it, so the
/// token — not the socket — is the boundary between users. Lower-case because HTTP header
/// names are case-insensitive and the `http` crate stores them lower-cased.
pub const LOCAL_AUTH_HEADER: &str = "x-soloist-local-auth";

/// How many random bytes back a [token](generate_token). 32 bytes (256 bits) is well past
/// any brute-force reach on a loopback socket, and it hex-encodes to a 64-character
/// printable header value.
const TOKEN_BYTES: usize = 32;

/// Mints a fresh per-launch auth token: [`TOKEN_BYTES`] of OS randomness, hex-encoded to a
/// printable header value. `None` if the OS randomness source is unavailable — the caller
/// then disables the API rather than fall back to a guessable token (fail closed).
pub fn generate_token() -> Option<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).ok()?;
    Some(hex::encode(bytes))
}

/// The status a mutation gets when the local-auth header is missing or wrong.
pub const STATUS_UNAUTHORIZED: u16 = 401;
/// The status a mutation gets when the command is not trusted (the core trust gate).
pub const STATUS_FORBIDDEN: u16 = 403;
/// The status a mutation gets when the named process or project does not exist.
pub const STATUS_NOT_FOUND: u16 = 404;

/// The file in the data directory recording the port the running server bound and its
/// per-launch token, so the CLI can reach and authenticate to it even after an
/// auto-fallback.
const RUNTIME_FILE: &str = "http-api.json";

/// Owner-only permissions (`rw-------`) for the runtime file. It carries the launch's auth
/// token, so it is kept unreadable to other local users. The data directory around it is
/// already `0700`, so this is defence in depth: it also holds even if a looser file were
/// left by an earlier build.
#[cfg(unix)]
const RUNTIME_FILE_MODE: u32 = 0o600;

/// What the running HTTP server records about itself for the CLI to read: where to reach it
/// and the token to present.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRuntime {
    /// The loopback port the server is listening on.
    pub port: u16,
    /// The per-launch token every request must carry (see [`LOCAL_AUTH_HEADER`]).
    pub token: String,
}

/// The body of `POST /projects/:id/spawn-agent`: which configured agent tool to launch as a
/// worker in the project, and any extra arguments appended to its command line for this run.
/// One definition the CLI serialises and the in-app server deserialises, so the wire shape
/// stays single-source (like [`HttpRuntime`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnRequest {
    /// The agent tool to launch — an entry in the app's agent-tool registry (e.g. `"Claude"`).
    pub tool: String,
    /// Extra arguments appended to the tool's command line for this launch.
    #[serde(default)]
    pub args: Vec<String>,
}

/// The response to a successful `POST /projects/:id/spawn-agent`: the id of the newly spawned,
/// started agent process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnResponse {
    /// The new agent process's id.
    pub id: u64,
}

/// The body of `POST /projects/:id/transfer-todo`: which todo in the path (source) project to move,
/// and the project to move it to. One definition the in-app server deserialises (a future CLI would
/// serialise it), so the wire shape stays single-source (like [`SpawnRequest`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferTodoRequest {
    /// The id of the todo to move, as it exists in the path (source) project.
    pub todo: u64,
    /// The id of the project to move it to.
    pub to_project: u64,
}

/// The body of `POST /projects/:id/transfer-scratchpad`: which scratchpad in the path (source)
/// project to move, by its name handle, and the project to move it to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferScratchpadRequest {
    /// The name handle of the scratchpad to move in the path (source) project.
    pub name: String,
    /// The id of the project to move it to.
    pub to_project: u64,
}

/// The runtime file's path inside the data directory (the directory is not created here).
pub fn runtime_file_path() -> Result<PathBuf, DataDirError> {
    Ok(data_dir()?.join(RUNTIME_FILE))
}

/// Records the port the server bound and its per-launch token, creating the (owner-only)
/// data directory first and writing the file owner-only (it holds the secret). The single
/// writer is the running server.
pub fn write_runtime(runtime: HttpRuntime) -> io::Result<()> {
    let path = ensure_data_dir()?.join(RUNTIME_FILE);
    let json = serde_json::to_vec(&runtime)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    write_owner_only(&path, &json)
}

/// Writes `bytes` to `path` restricted to its owner, so the token it carries stays
/// unreadable to other local users. Creating with the mode closes the window a
/// write-then-chmod would open; the explicit `set_permissions` then also tightens a file an
/// earlier build may have left looser (the create mode applies only to a new file).
#[cfg(unix)]
fn write_owner_only(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(RUNTIME_FILE_MODE)
        .open(path)?;
    file.set_permissions(std::fs::Permissions::from_mode(RUNTIME_FILE_MODE))?;
    file.write_all(bytes)
}

/// The non-Unix fallback: a plain write, since the owner-only mode is a Unix concept and the
/// supported target is Linux.
#[cfg(not(unix))]
fn write_owner_only(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(path, bytes)
}

/// Reads the running server's recorded runtime, or `None` when the file is absent or
/// unreadable — the CLI treats a missing file as "try the default port" and a refused
/// connection as "Soloist is not running". The server rewrites the file on every bind, so
/// a present file always names the current port.
pub fn read_runtime() -> Option<HttpRuntime> {
    let path = runtime_file_path().ok()?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Removes the runtime file on a graceful shutdown, so a stale port does not outlive the
/// server. Best-effort: a missing file or unresolved data directory is fine, since a refused
/// connection already reads as "Soloist is not running" and the next bind rewrites the file.
pub fn remove_runtime() {
    if let Ok(path) = runtime_file_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
