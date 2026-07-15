//! The loopback HTTP API adapter: an `axum` server bound to `127.0.0.1` that drives the
//! same [`Facade`] the desktop UI and the MCP server use, so a shell or launcher controls
//! the stack identically. Every route requires the launch's auth token and a loopback
//! `Host`, and CORS is restricted to localhost. The server holds no business state — every
//! route maps to one façade call.

mod auth;
mod cors;
mod host;
mod mutations;
mod routes;
mod state;

use std::future::Future;
use std::sync::Arc;

use soloist_core::Facade;
use soloist_ipc::http::{generate_token, write_runtime, HttpRuntime, DEFAULT_PORT};
use tokio::net::TcpListener;

pub use routes::router;
pub use soloist_ipc::http::remove_runtime;
pub use state::{ApiState, FocusFn};

/// How many ports above [`DEFAULT_PORT`] to try before asking the OS for any free port.
const FALLBACK_TRIES: u16 = 16;

/// Binds the loopback HTTP API and serves it until `shutdown` resolves (a live disable of the
/// integration, or app shutdown). Degrades to a logged no-op if no loopback port can be bound, so
/// a port conflict disables the API rather than taking down the app (graceful degradation).
/// Records the bound port so the CLI can reach it after a fallback, and drops that record when it
/// stops so the CLI never targets a server that is no longer listening. `focus` raises the desktop
/// window for `POST /focus` — the one effect that cannot route through the core, so the composition
/// root supplies it.
pub async fn serve(
    facade: Arc<Facade>,
    focus: FocusFn,
    shutdown: impl Future<Output = ()> + Send + 'static,
) {
    let Some(listener) = bind_loopback().await else {
        eprintln!("soloist: HTTP API disabled (no loopback port available)");
        return;
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(err) => {
            eprintln!("soloist: HTTP API disabled (cannot read the bound address: {err})");
            return;
        }
    };
    // Mint the launch's token before serving. Without OS randomness there is no safe token,
    // so disable the API rather than serve a guessable one (fail closed).
    let Some(token) = generate_token() else {
        eprintln!("soloist: HTTP API disabled (no OS randomness for the auth token)");
        return;
    };
    if let Err(err) = write_runtime(HttpRuntime {
        port,
        token: token.clone(),
    }) {
        // The server still requires the token; only the CLI's discovery file is missing, so
        // the CLI cannot authenticate until the next successful write. Better than serving
        // ungated.
        eprintln!("soloist: could not record the HTTP API runtime file ({err})");
    }
    let state = ApiState::new(facade, token).with_focus(focus);
    serve_on(listener, state, shutdown).await;
    // The server has stopped (a live disable, or app exit): drop the port record so the CLI does
    // not target a dead port. A re-enable rewrites it on the next bind.
    remove_runtime();
}

/// Serves the API on an already-bound listener until `shutdown` resolves, then stops accepting and
/// drains in-flight requests before returning (axum's graceful shutdown). Split out so a test can
/// drive it on an ephemeral port without binding the loopback default.
pub(crate) async fn serve_on(
    listener: TcpListener,
    state: ApiState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) {
    if let Err(err) = axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await
    {
        eprintln!("soloist: HTTP API stopped: {err}");
    }
}

/// Tries [`DEFAULT_PORT`], then the next [`FALLBACK_TRIES`] ports, then any OS-assigned
/// port, all on `127.0.0.1`. Returns the first that binds, or `None` if none did.
async fn bind_loopback() -> Option<TcpListener> {
    for offset in 0..=FALLBACK_TRIES {
        let port = DEFAULT_PORT.saturating_add(offset);
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return Some(listener);
        }
    }
    TcpListener::bind(("127.0.0.1", 0)).await.ok()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
