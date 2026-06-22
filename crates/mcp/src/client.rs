//! The IPC client: the MCP server's single, persistent connection to the running app.
//!
//! All tool calls share one connection so the app attributes them to one identity session
//! (bound once, on connect, to the process Soloist launched us in). The connection is
//! opened lazily and reopened after a transport failure, so the MCP server starts and lists
//! its tools even when the app is not running — a tool call then returns a clear
//! "Soloist not running" error.

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use soloist_core::ProcessId;
use soloist_ipc::{read_frame, write_frame, IpcError, IpcRequest, IpcResponse, IpcResult};
use tokio::net::UnixStream;
use tokio::sync::Mutex;

/// How long a single request waits for the app before the connection is treated as wedged.
/// A backstop against a hung app, not a per-tool deadline: a local request answers in
/// milliseconds, so this only fires when the app has stopped responding — bounding the call
/// (and the shared connection behind it) instead of blocking the MCP host forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Why a request to the app failed.
#[derive(Debug)]
pub enum ClientError {
    /// The app's IPC socket could not be reached — Soloist is not running.
    NotRunning,
    /// The app did not answer within [`REQUEST_TIMEOUT`].
    Timeout,
    /// The connection failed mid-request.
    Transport,
    /// The app served the request but returned a typed error.
    App(IpcError),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::NotRunning => {
                write!(f, "Soloist is not running (could not reach its IPC socket)")
            }
            ClientError::Timeout => write!(f, "Soloist did not respond in time"),
            ClientError::Transport => write!(f, "lost the connection to Soloist"),
            ClientError::App(err) => write!(f, "{err}"),
        }
    }
}

/// A stateless front over one connection to the app.
pub struct AppClient {
    /// The app's IPC socket path.
    socket: PathBuf,
    /// The process Soloist launched us in, bound on each fresh connection so the app
    /// attributes our calls to it.
    bound: Option<ProcessId>,
    /// The live connection, opened lazily and reopened after a transport failure.
    stream: Mutex<Option<UnixStream>>,
}

impl AppClient {
    /// A client that talks to the app on `socket`, binding each fresh connection to `bound`
    /// (the `SOLOIST_PROCESS_ID` Soloist injected) — or to nothing when launched outside it.
    pub fn new(bound: Option<ProcessId>, socket: PathBuf) -> Self {
        Self {
            socket,
            bound,
            stream: Mutex::new(None),
        }
    }

    /// Sends one request and returns the app's response, (re)connecting if needed.
    pub async fn request(&self, request: IpcRequest) -> Result<IpcResponse, ClientError> {
        let mut slot = self.stream.lock().await;
        if slot.is_none() {
            *slot = Some(self.connect().await?);
        }
        let stream = match slot.as_mut() {
            Some(stream) => stream,
            None => return Err(ClientError::NotRunning),
        };
        match exchange(stream, &request).await {
            Ok(reply) => reply.map_err(ClientError::App),
            Err(err) => {
                // Drop the broken connection so the next call reconnects.
                *slot = None;
                Err(err)
            }
        }
    }

    /// Opens a fresh connection and binds it to our process, best-effort.
    async fn connect(&self) -> Result<UnixStream, ClientError> {
        let mut stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|_| ClientError::NotRunning)?;
        if let Some(process) = self.bound {
            // A bind failure must not fail the connection — whoami simply reports unbound.
            let _ = exchange(&mut stream, &IpcRequest::BindSessionProcess { process }).await;
        }
        Ok(stream)
    }
}

/// Writes one request and reads one reply over the stream, bounded by [`REQUEST_TIMEOUT`]
/// so a wedged app surfaces as [`ClientError::Timeout`] rather than hanging the caller.
async fn exchange(stream: &mut UnixStream, request: &IpcRequest) -> Result<IpcResult, ClientError> {
    let io = async {
        write_frame(stream, request)
            .await
            .map_err(|_| ClientError::Transport)?;
        read_frame::<_, IpcResult>(stream)
            .await
            .map_err(|_| ClientError::Transport)?
            .ok_or(ClientError::Transport)
    };
    tokio::time::timeout(REQUEST_TIMEOUT, io)
        .await
        .map_err(|_| ClientError::Timeout)?
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
