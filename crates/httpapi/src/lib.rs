//! The loopback HTTP API adapter: an `axum` server bound to `127.0.0.1` that drives the
//! same [`Facade`] the desktop UI and the MCP server use, so a shell or launcher controls
//! the stack identically. Read routes are open on loopback; mutations require an auth
//! header and CORS is restricted to localhost. The server holds no business state — every
//! route maps to one façade call.

mod auth;
mod cors;
mod mutations;
mod routes;
mod state;

use std::sync::Arc;

use soloist_core::Facade;
use soloist_ipc::http::{write_runtime, HttpRuntime, DEFAULT_PORT};
use tokio::net::TcpListener;

pub use routes::router;
pub use state::{ApiState, FocusFn};

/// How many ports above [`DEFAULT_PORT`] to try before asking the OS for any free port.
const FALLBACK_TRIES: u16 = 16;

/// Binds the loopback HTTP API and serves it until the app shuts down. Degrades to a
/// logged no-op if no loopback port can be bound, so a port conflict disables the API
/// rather than taking down the app (graceful degradation). Records the bound port so the
/// CLI can reach it after a fallback. `focus` raises the desktop window for `POST /focus` —
/// the one effect that cannot route through the core, so the composition root supplies it.
pub async fn serve(facade: Arc<Facade>, focus: FocusFn) {
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
    if let Err(err) = write_runtime(HttpRuntime { port }) {
        // The server still serves; only the CLI's port-discovery file is missing, and the
        // CLI falls back to the default port.
        eprintln!("soloist: could not record the HTTP API port ({err})");
    }
    let state = ApiState::new(facade).with_focus(focus);
    if let Err(err) = axum::serve(listener, router(state)).await {
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
