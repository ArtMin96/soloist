//! The local HTTP API's shared contract: the loopback port, the mutation auth header,
//! and the runtime file recording the port the server actually bound.
//!
//! Defined here, in the transport crate, so the two halves agree without one telling the
//! other: the in-app server (`soloist-httpapi`) writes the runtime file after it binds,
//! and the `soloist` CLI reads it to find the port. The header and its value are the one
//! definition the server checks and the client sends.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths::{data_dir, ensure_data_dir, DataDirError};

/// The loopback port the HTTP API prefers. The server falls back to a nearby port if it
/// is taken and records the chosen one in the [runtime file](runtime_file_path).
pub const DEFAULT_PORT: u16 = 24678;

/// The header a mutating request must carry. Loopback bind plus localhost CORS keep
/// remote and cross-origin callers out; this header is the deliberate, weak local gate
/// that stops a drive-by request from a page the user merely happens to be viewing.
/// Lower-case because HTTP header names are case-insensitive and the `http` crate stores
/// them lower-cased.
pub const LOCAL_AUTH_HEADER: &str = "x-soloist-local-auth";

/// The value [`LOCAL_AUTH_HEADER`] must hold on a mutating request.
pub const LOCAL_AUTH_VALUE: &str = "1";

/// The file in the data directory recording the port the running server bound, so the
/// CLI can reach it even after an auto-fallback.
const RUNTIME_FILE: &str = "http-api.json";

/// What the running HTTP server records about itself for the CLI to read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRuntime {
    /// The loopback port the server is listening on.
    pub port: u16,
}

/// The runtime file's path inside the data directory (the directory is not created here).
pub fn runtime_file_path() -> Result<PathBuf, DataDirError> {
    Ok(data_dir()?.join(RUNTIME_FILE))
}

/// Records the port the server bound, creating the (owner-only) data directory first.
/// The single writer is the running server.
pub fn write_runtime(runtime: HttpRuntime) -> io::Result<()> {
    let path = ensure_data_dir()?.join(RUNTIME_FILE);
    let json = serde_json::to_vec(&runtime)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    std::fs::write(path, json)
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
