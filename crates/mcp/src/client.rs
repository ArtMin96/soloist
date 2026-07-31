//! The IPC client: the MCP server's single, persistent connection to the running app.
//!
//! All tool calls share one connection so the app attributes them to one identity session
//! (bound once, on connect, to the process Soloist launched us in). The connection is
//! opened lazily and reopened after a transport failure, so the MCP server starts and lists
//! its tools even when the app is not running — a tool call then returns a clear
//! "Soloist not running" error.

use std::fmt;
use std::path::PathBuf;
use std::sync::PoisonError;
use std::time::Duration;

use soloist_core::{McpToolGroups, ProcessId};
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

/// Why the bind on a fresh connection did not take effect. The session stays usable — it simply
/// runs unbound — so this is a diagnostic, never a connection failure.
#[derive(Clone, Debug, PartialEq, Eq)]
enum BindFailure {
    /// The app answered the bind with a typed refusal.
    Refused(IpcError),
    /// The app could not be asked — the bind request did not complete.
    Unreachable,
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
    /// The bind failure already written to stderr, so a refusal that persists across reconnects
    /// is stated once rather than on every connection. A plain `std::sync::Mutex`: the critical
    /// section only compares and swaps a value, and nothing is awaited while it is held.
    reported_bind_failure: std::sync::Mutex<Option<BindFailure>>,
}

impl AppClient {
    /// A client that talks to the app on `socket`, binding each fresh connection to `bound`
    /// (the `SOLOIST_PROCESS_ID` Soloist injected) — or to nothing when launched outside it.
    pub fn new(bound: Option<ProcessId>, socket: PathBuf) -> Self {
        Self {
            socket,
            bound,
            stream: Mutex::new(None),
            reported_bind_failure: std::sync::Mutex::new(None),
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
            // A bind failure must not fail the connection — the session runs unbound instead, and
            // the app records the refusal for `whoami`. It is reported here as well, so a user
            // reading the MCP host's log learns of it without having to ask an agent.
            if let Some(report) = self.bind_session(&mut stream, process).await {
                eprintln!("{report}");
            }
        }
        Ok(stream)
    }

    /// Binds a fresh connection to `process`, returning the line to write to stderr when the
    /// outcome is worth stating.
    async fn bind_session(&self, stream: &mut UnixStream, process: ProcessId) -> Option<String> {
        let outcome = match exchange(stream, &IpcRequest::BindSessionProcess { process }).await {
            Ok(Ok(_)) => None,
            Ok(Err(err)) => Some(BindFailure::Refused(err)),
            Err(_) => Some(BindFailure::Unreachable),
        };
        self.note_bind_outcome(process, outcome)
    }

    /// Records a connection's bind `outcome` and returns the line to report, or `None` when there
    /// is nothing new to say. Repeating the failure already reported is silent, so a standing
    /// refusal is stated once across reconnects; a bind that *succeeds* forgets it, so the same
    /// refusal arriving after an intervening success is news again rather than a repeat — that is
    /// a session that went unbound, which is the event this line exists to announce.
    fn note_bind_outcome(
        &self,
        process: ProcessId,
        outcome: Option<BindFailure>,
    ) -> Option<String> {
        let mut reported = self
            .reported_bind_failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(failure) = outcome else {
            *reported = None;
            return None;
        };
        if reported.as_ref() == Some(&failure) {
            return None;
        }
        let report = bind_failure_report(process, &failure);
        *reported = Some(failure);
        Some(report)
    }
}

/// The diagnostic for a bind that did not take effect: the process the session could not bind to,
/// why, and what to do about it. Written to stderr, which an MCP host captures for its user — the
/// one channel a stdio server has that does not disturb the protocol on stdout.
fn bind_failure_report(process: ProcessId, failure: &BindFailure) -> String {
    let (reason, remedy) = match failure {
        BindFailure::Refused(err) => (err.to_string(), retry_remedy(err)),
        BindFailure::Unreachable => ("Soloist did not answer".to_string(), Some(RETRY_REMEDY)),
    };
    let mut report = format!(
        "soloist-mcp: could not bind this session to process {process}: {reason}. \
Tools run unbound, so anything that needs an owning process (leases, timers) is refused."
    );
    if let Some(remedy) = remedy {
        report.push(' ');
        report.push_str(remedy);
    }
    report
}

/// Whether a refused bind is worth retrying. `bind_session_process` re-runs the very check that
/// produced the refusal, so the retry is only worth naming where that check can come out
/// differently: a process the app does not know may yet register, but the peer process group
/// behind [`IpcError::ForeignProcess`] is read once per connection and cannot change under it —
/// and that refusal states its own way forward.
fn retry_remedy(err: &IpcError) -> Option<&'static str> {
    match err {
        IpcError::ForeignProcess => None,
        _ => Some(RETRY_REMEDY),
    }
}

/// The way back where a retry can genuinely take: the bind never reached the app, or named a
/// process that may have registered since.
const RETRY_REMEDY: &str = "Call `whoami` for the recorded reason, and `bind_session_process` \
with the injected id to retry.";

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
