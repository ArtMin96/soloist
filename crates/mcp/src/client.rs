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

use soloist_core::{McpToolGroups, ProcessId};
use soloist_ipc::{
    read_frame, write_frame, IpcError, IpcReply, IpcRequest, IpcResponse, IpcResult,
};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout_at, Instant};

/// How long a single request waits for the app to say **anything** before the connection is treated
/// as wedged. A backstop against a hung app, not a per-tool deadline: a local request answers in
/// milliseconds, so this only fires when the app has stopped responding — bounding the call
/// (and the shared connection behind it) instead of blocking the MCP host forever.
///
/// A request that asked to be told what is happening resets this every time it is told, because a
/// remark is the app saying it is still working; one that did not spends it exactly once, as it
/// always has.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The longest a single request may take however much it keeps reporting.
///
/// Progress that resets a deadline without a ceiling is a deadline that can be talked out of ever
/// arriving, so there is one regardless — the same reasoning the protocol gives for it: an
/// implementation "SHOULD always enforce a maximum timeout, regardless of progress notifications, to
/// limit the impact of a misbehaving client or server". Set well above the bound the app puts on any
/// single operation it runs, so it is reached only by something that has gone wrong rather than by
/// slow legitimate work.
const MAX_REPORTED_WAIT: Duration = Duration::from_secs(300);

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
    /// How long one request tolerates hearing nothing at all, and the most it may take however much
    /// it hears. Held rather than read from the constants at the point of use so a test about the
    /// waiting can set bounds it can reach, instead of taking minutes to reach the real ones.
    silence: Duration,
    ceiling: Duration,
}

impl AppClient {
    /// A client that talks to the app on `socket`, binding each fresh connection to `bound`
    /// (the `SOLOIST_PROCESS_ID` Soloist injected) — or to nothing when launched outside it.
    pub fn new(bound: Option<ProcessId>, socket: PathBuf) -> Self {
        Self {
            socket,
            bound,
            stream: Mutex::new(None),
            silence: REQUEST_TIMEOUT,
            ceiling: MAX_REPORTED_WAIT,
        }
    }

    /// The same client, waiting on the bounds a test can reach.
    #[cfg(test)]
    pub(crate) fn waiting(mut self, silence: Duration, ceiling: Duration) -> Self {
        self.silence = silence;
        self.ceiling = ceiling;
        self
    }

    /// Sends one request and returns the app's response, (re)connecting if needed.
    pub async fn request(&self, request: IpcRequest) -> Result<IpcResponse, ClientError> {
        self.send(request, None).await
    }

    /// The same, for a request that asked the app to say what it is doing while it does it: each
    /// remark is handed to `reports` as it arrives, and the wait for the answer is renewed by it.
    ///
    /// Remarks are dropped rather than queued when `reports` is full — one that arrives late is
    /// worth less than the one after it — so nothing about being watched can slow the request down.
    pub async fn request_reporting(
        &self,
        request: IpcRequest,
        reports: mpsc::Sender<String>,
    ) -> Result<IpcResponse, ClientError> {
        self.send(request, Some(reports)).await
    }

    /// The one exchange every request makes, whether or not anybody asked to be told about it.
    async fn send(
        &self,
        request: IpcRequest,
        reports: Option<mpsc::Sender<String>>,
    ) -> Result<IpcResponse, ClientError> {
        let mut slot = self.stream.lock().await;
        if slot.is_none() {
            *slot = Some(self.connect().await?);
        }
        let stream = match slot.as_mut() {
            Some(stream) => stream,
            None => return Err(ClientError::NotRunning),
        };
        match exchange(
            stream,
            &request,
            reports.as_ref(),
            self.silence,
            self.ceiling,
        )
        .await
        {
            Ok(reply) => reply.map_err(ClientError::App),
            Err(err) => {
                // Drop the broken connection so the next call reconnects.
                *slot = None;
                Err(err)
            }
        }
    }

    /// Reads the MCP feature-group enablement from the app — the server consults this at startup to
    /// decide which feature-tool groups to serve. Any failure (app down, transport, or a mismatched
    /// reply) is the caller's to handle; the server falls back to the defaults so it still starts
    /// when the app is unreachable.
    pub async fn mcp_tool_groups(&self) -> Result<McpToolGroups, ClientError> {
        match self.request(IpcRequest::McpToolGroups).await? {
            IpcResponse::McpToolGroups(groups) => Ok(groups),
            _ => Err(ClientError::Transport),
        }
    }

    /// Opens a fresh connection and binds it to our process, best-effort.
    async fn connect(&self) -> Result<UnixStream, ClientError> {
        let mut stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|_| ClientError::NotRunning)?;
        if let Some(process) = self.bound {
            // A bind failure must not fail the connection — whoami simply reports unbound.
            let _ = exchange(
                &mut stream,
                &IpcRequest::BindSessionProcess { process },
                None,
                self.silence,
                self.ceiling,
            )
            .await;
        }
        Ok(stream)
    }
}

/// Writes one request and reads frames until the one that answers it, bounded by
/// [`REQUEST_TIMEOUT`] so a wedged app surfaces as [`ClientError::Timeout`] rather than hanging the
/// caller.
///
/// A request nobody asked to be told about spends that budget exactly once, across the write and the
/// single frame that answers it — which is what it has always done. One that did asked renews it on
/// every remark, because a remark is evidence the app is working rather than wedged, and is bounded
/// regardless by [`MAX_REPORTED_WAIT`]. A remark arriving for a request that never asked for one is
/// a server not keeping the protocol, and the connection is treated as broken.
async fn exchange(
    stream: &mut UnixStream,
    request: &IpcRequest,
    reports: Option<&mpsc::Sender<String>>,
    silence: Duration,
    ceiling: Duration,
) -> Result<IpcResult, ClientError> {
    let ceiling = Instant::now() + ceiling;
    let mut deadline = Instant::now() + silence;
    timeout_at(deadline, write_frame(stream, request))
        .await
        .map_err(|_| ClientError::Timeout)?
        .map_err(|_| ClientError::Transport)?;
    loop {
        let frame = timeout_at(deadline, read_frame::<_, IpcReply>(stream))
            .await
            .map_err(|_| ClientError::Timeout)?
            .map_err(|_| ClientError::Transport)?
            .ok_or(ClientError::Transport)?;
        let report = match frame {
            IpcReply::Done(reply) => return Ok(*reply),
            IpcReply::Progress(report) => report,
        };
        let Some(reports) = reports else {
            return Err(ClientError::Transport);
        };
        deadline = (Instant::now() + silence).min(ceiling);
        if deadline <= Instant::now() {
            return Err(ClientError::Timeout);
        }
        let _ = reports.try_send(report.note);
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
