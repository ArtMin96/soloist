use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use soloist_core::Integrations;
use tokio::net::TcpStream;
use tokio_util::sync::CancellationToken;

use super::{IntegrationServers, ToggleableServer};

/// A stand-in server: it binds an ephemeral loopback port synchronously (so the port is live the
/// moment a start returns), publishes it into `slot` while running, accepts until cancelled, then
/// frees the port. This lets the tests observe start/stop over a real socket, without the real
/// MCP/HTTP adapters or a Tauri app.
fn fake_server(slot: Arc<Mutex<Option<SocketAddr>>>) -> ToggleableServer {
    ToggleableServer::new("fake", move |token: CancellationToken| {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        *slot.lock().unwrap() = Some(listener.local_addr().expect("addr"));
        let slot = Arc::clone(&slot);
        async move {
            let listener = tokio::net::TcpListener::from_std(listener).expect("from_std");
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    accepted = listener.accept() => {
                        let _ = accepted;
                    }
                }
            }
            // Cancelled: mark the port freed and drop the listener (end of scope).
            *slot.lock().unwrap() = None;
        }
    })
}

fn bound_addr(slot: &Arc<Mutex<Option<SocketAddr>>>) -> Option<SocketAddr> {
    *slot.lock().unwrap()
}

// The startup gate: a server that boots disabled never binds its socket.
#[tokio::test]
async fn a_disabled_server_never_binds() {
    let slot = Arc::new(Mutex::new(None));
    let mut server = fake_server(Arc::clone(&slot));
    server.set(false).await;
    assert!(
        bound_addr(&slot).is_none(),
        "a server left off must not bind a socket"
    );
}

// The live teardown: enabling binds and accepts; disabling frees the socket, which then refuses.
#[tokio::test]
async fn enabling_binds_and_disabling_frees_the_port() {
    let slot = Arc::new(Mutex::new(None));
    let mut server = fake_server(Arc::clone(&slot));

    server.set(true).await;
    let addr = bound_addr(&slot).expect("bound while enabled");
    TcpStream::connect(addr)
        .await
        .expect("connect while enabled");

    server.set(false).await;
    assert!(bound_addr(&slot).is_none(), "disabling frees the port");
    assert!(
        TcpStream::connect(addr).await.is_err(),
        "the freed port must refuse new connections"
    );
}

// The respawn: after a disable, re-enabling brings the server back up and serving.
#[tokio::test]
async fn re_enabling_respawns_a_serving_socket() {
    let slot = Arc::new(Mutex::new(None));
    let mut server = fake_server(Arc::clone(&slot));

    server.set(true).await;
    server.set(false).await;
    server.set(true).await;

    let addr = bound_addr(&slot).expect("bound after respawn");
    TcpStream::connect(addr)
        .await
        .expect("connect after respawn");
}

// Idempotency: re-applying "enabled" keeps the same live run rather than restarting it.
#[tokio::test]
async fn re_enabling_keeps_the_same_run() {
    let slot = Arc::new(Mutex::new(None));
    let mut server = fake_server(Arc::clone(&slot));

    server.set(true).await;
    let addr = bound_addr(&slot).expect("bound");
    server.set(true).await;
    assert_eq!(
        bound_addr(&slot),
        Some(addr),
        "re-enabling must not respawn the server on a new port"
    );
}

// Routing: apply maps mcp_enabled to the MCP slot and http_api_enabled to the HTTP slot, live and
// in both directions.
#[tokio::test]
async fn apply_routes_each_toggle_to_its_own_server() {
    let mcp = Arc::new(Mutex::new(None));
    let http = Arc::new(Mutex::new(None));
    let servers = IntegrationServers::new(
        Some(fake_server(Arc::clone(&mcp))),
        Some(fake_server(Arc::clone(&http))),
    );

    servers
        .apply(Integrations {
            mcp_enabled: false,
            http_api_enabled: true,
        })
        .await;
    assert!(
        bound_addr(&mcp).is_none(),
        "mcp off must not bind the MCP server"
    );
    assert!(
        bound_addr(&http).is_some(),
        "http on must bind the HTTP server"
    );

    servers
        .apply(Integrations {
            mcp_enabled: true,
            http_api_enabled: false,
        })
        .await;
    assert!(bound_addr(&mcp).is_some(), "toggling MCP on binds it live");
    assert!(
        bound_addr(&http).is_none(),
        "toggling HTTP off frees it live"
    );
}

// The exit path: shutting the set down frees every socket it holds, so nothing the app was
// serving outlives it.
#[tokio::test]
async fn shutdown_frees_every_bound_socket() {
    let mcp = Arc::new(Mutex::new(None));
    let http = Arc::new(Mutex::new(None));
    let servers = IntegrationServers::new(
        Some(fake_server(Arc::clone(&mcp))),
        Some(fake_server(Arc::clone(&http))),
    );
    servers.apply(Integrations::default()).await;
    let mcp_addr = bound_addr(&mcp).expect("mcp bound");
    let http_addr = bound_addr(&http).expect("http bound");

    servers.shutdown().await;

    assert!(bound_addr(&mcp).is_none(), "shutdown frees the MCP socket");
    assert!(
        bound_addr(&http).is_none(),
        "shutdown frees the HTTP socket"
    );
    assert!(
        TcpStream::connect(mcp_addr).await.is_err(),
        "the freed MCP port must refuse new connections"
    );
    assert!(
        TcpStream::connect(http_addr).await.is_err(),
        "the freed HTTP port must refuse new connections"
    );
}

// The terminal state: a settings toggle racing the exit path cannot bring a server back up after
// the app has stopped it.
#[tokio::test]
async fn applying_settings_after_shutdown_never_rebinds() {
    let slot = Arc::new(Mutex::new(None));
    let servers = IntegrationServers::new(Some(fake_server(Arc::clone(&slot))), None);
    servers.apply(Integrations::default()).await;

    servers.shutdown().await;
    servers.apply(Integrations::default()).await;

    assert!(
        bound_addr(&slot).is_none(),
        "a shut-down set must stay down, whatever the settings say"
    );
}

// Idempotency: a second shutdown is harmless, so a repeated exit request cannot wedge the app.
#[tokio::test]
async fn shutting_down_twice_leaves_everything_stopped() {
    let slot = Arc::new(Mutex::new(None));
    let servers = IntegrationServers::new(Some(fake_server(Arc::clone(&slot))), None);
    servers.apply(Integrations::default()).await;

    servers.shutdown().await;
    servers.shutdown().await;

    assert!(bound_addr(&slot).is_none(), "the socket stays freed");
}
