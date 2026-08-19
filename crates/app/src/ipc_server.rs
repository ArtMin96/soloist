//! The local IPC server: the Unix-socket front the MCP server (`soloist-mcp`) connects to.
//!
//! This is the app-side half of the [`soloist_ipc`] transport — a driving adapter compiled
//! in only under the `mcp` feature, so turning the feature off drops it (and its dependency)
//! and the app still builds and runs. This module is the transport itself: it binds and serves
//! the socket, and gives each connection one identity session. What a request *means* is the
//! [`dispatch`] module's job; the server holds no business state.

use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use crate::peer_cred;
use soloist_core::Facade;
use soloist_ipc::{
    ensure_socket_path, read_frame, write_frame, IpcReply, IpcRequest, ProgressReport,
};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
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

/// How many remarks about a running request may be waiting to be written before the oldest are
/// dropped rather than kept.
///
/// A remark is worth having only while it is current, so a connection that has fallen behind is
/// better served by the newest few than by a backlog it will never catch up on — and an operation
/// must never be slowed by whoever is watching it. Small on purpose: the source already coalesces
/// and rate-limits, so anything past a handful means the connection is not keeping up at all.
const REPORT_BACKLOG: usize = 8;

/// Binds the IPC socket and serves connections until `shutdown` fires (a live disable of the
/// integration, or app shutdown), then unlinks the socket so a stopped server leaves nothing to
/// connect to. The same token reaches every accepted connection, so stopping covers the
/// connections already in hand as well as the ones not yet made.
/// Degrades to a logged no-op if the socket cannot be resolved or bound, so a packaging or
/// permissions problem disables MCP rather than taking down the app (graceful degradation).
pub async fn serve(facade: Arc<Facade>, shutdown: CancellationToken) {
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
                tauri::async_runtime::spawn(handle_connection(
                    Arc::clone(&facade),
                    stream,
                    shutdown.clone(),
                ));
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
/// session with them, answers framed requests until the peer disconnects or `shutdown` fires, then
/// closes the session so its scope and binding are forgotten. The peer's group and working
/// directory are what authenticate a session's project scope — the core matches the group to the
/// managed process the caller runs in, and the directory to the project root it runs under — so a
/// client cannot bind to or act on a sibling project it does not run in. A connection whose peer
/// credentials cannot be read, or whose peer is a different UID than Soloist runs as, is dropped
/// (fail closed).
async fn handle_connection(
    facade: Arc<Facade>,
    mut stream: UnixStream,
    shutdown: CancellationToken,
) {
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
    let session = facade.open_session(credentials);
    loop {
        // Only the *next* request is raced against the stop: a request already being served runs
        // to its answer, so stopping never abandons work the caller was told had started. Biased
        // so a stop always wins over a request that arrived in the same instant — otherwise which
        // of the two the connection honours would be a coin toss.
        let request: IpcRequest = tokio::select! {
            biased;
            () = shutdown.cancelled() => break,
            frame = read_frame(&mut stream) => match frame {
                Ok(Some(request)) => request,
                Ok(None) => break, // the peer closed the connection
                Err(err) => {
                    eprintln!("soloist: MCP IPC read error: {err}");
                    break;
                }
            },
        };
        let (reports, mut reported) = mpsc::channel(REPORT_BACKLOG);
        // Remarks and the answer share one connection, so one loop writes both: whatever the request
        // says about itself goes out as its own frame while it runs, and the answer that ends it is
        // always the last frame written for it.
        let reply = {
            let mut serving = pin!(handle_request(&facade, session, request, reports));
            let mut written = Ok(());
            let answer = loop {
                let remark = tokio::select! {
                    answer = &mut serving => break answer,
                    Some(remark) = reported.recv() => remark,
                };
                written = write_frame(
                    &mut stream,
                    &IpcReply::Progress(ProgressReport { note: remark }),
                )
                .await;
                if written.is_err() {
                    break serving.await;
                }
            };
            if let Err(err) = written {
                eprintln!("soloist: MCP IPC write error: {err}");
                break;
            }
            answer
        };
        if let Err(err) = write_frame(&mut stream, &IpcReply::Done(Box::new(reply))).await {
            eprintln!("soloist: MCP IPC write error: {err}");
            break;
        }
    }
    facade.scoped(session).close_session();
}

#[cfg(test)]
#[path = "ipc_server_tests.rs"]
mod tests;
