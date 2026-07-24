//! The local IPC server: the Unix-socket front the MCP server (`soloist-mcp`) connects to.
//!
//! This is the app-side half of the [`soloist_ipc`] transport — a driving adapter compiled
//! in only under the `mcp` feature, so turning the feature off drops it (and its dependency)
//! and the app still builds and runs. This module is the transport itself: it binds and serves
//! the socket, and gives each connection one identity session. What a request *means* is the
//! [`dispatch`] module's job; the server holds no business state.

use std::sync::Arc;
use std::time::Duration;

use crate::peer_cred;
use soloist_core::Facade;
use soloist_ipc::{ensure_socket_path, read_frame, write_frame, IpcRequest};
use tauri::{AppHandle, Manager};
use tokio::net::{UnixListener, UnixStream};
use tokio_util::sync::CancellationToken;

mod dispatch;

use dispatch::handle_request;

/// Backoff after a transient `accept` failure, so a persistent condition (e.g. FD exhaustion)
/// cannot hot-loop the accept task while it keeps serving.
const ACCEPT_RETRY_BACKOFF: Duration = Duration::from_millis(100);
/// The most consecutive `accept` failures tolerated before the front gives up and degrades to a
/// logged no-op. A transient condition clears well within this many backed-off retries; one that
/// never clears is bounded here rather than retried forever (no retry without a ceiling).
const MAX_CONSECUTIVE_ACCEPT_ERRORS: u32 = 64;

/// Binds the IPC socket and serves connections until `shutdown` fires (a live disable of the
/// integration, or app shutdown), then unlinks the socket so a disabled server leaves nothing to
/// connect to; already-accepted connections keep their own descriptors and drain on their own.
/// Degrades to a logged no-op if the socket cannot be resolved or bound, so a packaging or
/// permissions problem disables MCP rather than taking down the app (graceful degradation).
pub async fn serve(app: AppHandle, shutdown: CancellationToken) {
    // Resolves the socket path and creates its owner-only data directory in one step — the
    // single resolution the store shares, so the socket and database keep one private home.
    let path = match ensure_socket_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("soloist: MCP IPC disabled (cannot prepare the socket directory: {err})");
            return;
        }
    };
    // A leftover socket from a previous run would make bind fail; the path is ours to clear.
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!(
                "soloist: MCP IPC disabled (cannot bind {}: {err})",
                path.display()
            );
            return;
        }
    };
    let mut consecutive_errors: u32 = 0;
    loop {
        let accepted = tokio::select! {
            _ = shutdown.cancelled() => break,
            accepted = listener.accept() => accepted,
        };
        match accepted {
            Ok((stream, _addr)) => {
                consecutive_errors = 0;
                tauri::async_runtime::spawn(handle_connection(app.clone(), stream));
            }
            Err(err) if accept_error_is_fatal(&err) => {
                // The listener socket itself is unusable; retrying accept on it can never
                // succeed, so degrade to a logged no-op rather than hot-loop forever.
                eprintln!("soloist: MCP IPC disabled (unrecoverable accept error: {err})");
                return;
            }
            Err(err) => {
                // A transient accept error — FD pressure (EMFILE/ENFILE) in a PTY-heavy
                // supervisor, or a peer that aborted before we accepted it — must not tear
                // down the whole MCP front, or every agent sees "Soloist is not running"
                // until the app restarts. Back off briefly so it cannot hot-loop, and keep
                // serving — up to a ceiling, so a condition that never clears is bounded.
                consecutive_errors += 1;
                if consecutive_errors >= MAX_CONSECUTIVE_ACCEPT_ERRORS {
                    eprintln!(
                        "soloist: MCP IPC disabled (accept kept failing after \
                         {consecutive_errors} retries: {err})"
                    );
                    return;
                }
                eprintln!(
                    "soloist: MCP IPC accept error \
                     (retry {consecutive_errors}/{MAX_CONSECUTIVE_ACCEPT_ERRORS}): {err}"
                );
                tokio::time::sleep(ACCEPT_RETRY_BACKOFF).await;
            }
        }
    }
    // Shutdown requested: unlink the socket so a re-enabled server can rebind the same path and,
    // meanwhile, no client can connect to a server that has stopped accepting.
    let _ = std::fs::remove_file(&path);
}

/// Whether an `accept` error means the listener socket itself is unusable — retrying can never
/// succeed. Everything else (FD pressure `EMFILE`/`ENFILE`, an aborted peer `ECONNABORTED`,
/// transient kernel limits) is expected to clear and is retried with backoff.
fn accept_error_is_fatal(err: &std::io::Error) -> bool {
    matches!(
        err.raw_os_error(),
        Some(nix::libc::EBADF | nix::libc::EINVAL | nix::libc::ENOTSOCK | nix::libc::EOPNOTSUPP)
    )
}

/// Serves one client connection: reads the connecting peer's credentials, opens an identity
/// session with them, answers framed requests until the peer disconnects, then closes the
/// session so its scope and binding are forgotten. The peer's group and working directory are
/// what authenticate a session's project scope — the core matches the group to the managed
/// process the caller runs in, and the directory to the project root it runs under — so a client
/// cannot bind to or act on a sibling project it does not run in. A connection whose peer
/// credentials cannot be read, or whose peer is a different UID than Soloist runs as, is dropped
/// (fail closed).
async fn handle_connection(app: AppHandle, mut stream: UnixStream) {
    let resolved = peer_cred::peer_credentials(&stream);
    let credentials = match peer_cred::peer_scope(&resolved) {
        peer_cred::PeerScope::Open(credentials) => credentials,
        peer_cred::PeerScope::Drop => {
            // Credentials unreadable, or the peer is a different user — refuse either way.
            if let Err(err) = &resolved {
                eprintln!("soloist: MCP IPC dropped a connection ({err})");
            }
            return;
        }
    };
    let session = app.state::<Arc<Facade>>().open_session(credentials);
    loop {
        let request: IpcRequest = match read_frame(&mut stream).await {
            Ok(Some(request)) => request,
            Ok(None) => break, // the peer closed the connection
            Err(err) => {
                eprintln!("soloist: MCP IPC read error: {err}");
                break;
            }
        };
        let reply = handle_request(app.state::<Arc<Facade>>().inner(), session, request).await;
        if let Err(err) = write_frame(&mut stream, &reply).await {
            eprintln!("soloist: MCP IPC write error: {err}");
            break;
        }
    }
    app.state::<Arc<Facade>>().scoped(session).close_session();
}
