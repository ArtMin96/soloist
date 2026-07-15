//! The composition root's integration-server lifecycle. The MCP IPC server and the loopback HTTP
//! API are optional local surfaces the user can turn on or off; each is owned here as a running
//! task plus a cancellation handle, so toggling the durable Integrations setting starts or stops it
//! live — no app restart. The toggle command routes to the core setting; this applies that setting
//! to the sockets. No server behaviour lives here — only lifecycle.

use std::future::Future;
use std::pin::Pin;

use soloist_core::Integrations;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// The boxed future a running server drives; it must stop and resolve once its token is cancelled.
type BoxServerFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Builds a fresh run of a server, wired to stop when its [`CancellationToken`] fires. `Fn` (not
/// `FnOnce`) so the same server can be respawned after a disable→enable.
type SpawnServer = Box<dyn Fn(CancellationToken) -> BoxServerFuture + Send + Sync>;

/// One optional server that can be started or stopped at runtime. Holds the spawn recipe and, while
/// running, the live task's cancellation token and join handle.
pub(crate) struct ToggleableServer {
    name: &'static str,
    spawn: SpawnServer,
    running: Option<(CancellationToken, JoinHandle<()>)>,
}

#[cfg_attr(not(any(feature = "mcp", feature = "http")), allow(dead_code))]
impl ToggleableServer {
    /// A server named `name` (for logs) that each start runs via `spawn`, a closure producing the
    /// server's future from the cancellation token that stops it.
    pub(crate) fn new<F, Fut>(name: &'static str, spawn: F) -> Self
    where
        F: Fn(CancellationToken) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Self {
            name,
            spawn: Box::new(move |token| Box::pin(spawn(token))),
            running: None,
        }
    }

    /// Brings the server to `enabled`: spawns a fresh run if it should be up and is not, or cancels
    /// and drains the current run if it should be down and is. Already in the target state is a
    /// no-op, so re-applying unchanged settings does nothing.
    async fn set(&mut self, enabled: bool) {
        match (enabled, self.running.take()) {
            (true, None) => {
                let token = CancellationToken::new();
                let handle = tokio::spawn((self.spawn)(token.clone()));
                self.running = Some((token, handle));
            }
            // Already running: keep the live run rather than restart it.
            (true, run @ Some(_)) => self.running = run,
            (false, Some((token, handle))) => {
                token.cancel();
                if let Err(err) = handle.await {
                    eprintln!("soloist: {} did not stop cleanly ({err})", self.name);
                }
            }
            (false, None) => {}
        }
    }
}

/// The two integration servers, applied together from one durable Integrations setting. Held behind
/// a mutex so concurrent toggles serialize on the same handles. Either slot is absent when its
/// adapter is compiled out (or, in a test, replaced by a stand-in).
pub(crate) struct IntegrationServers {
    inner: Mutex<Inner>,
}

struct Inner {
    mcp: Option<ToggleableServer>,
    http: Option<ToggleableServer>,
}

impl IntegrationServers {
    pub(crate) fn new(mcp: Option<ToggleableServer>, http: Option<ToggleableServer>) -> Self {
        Self {
            inner: Mutex::new(Inner { mcp, http }),
        }
    }

    /// Applies the settings to the live sockets: each present server is started or stopped to match
    /// its toggle. Called once at boot (a disabled server never binds) and again on every change to
    /// the Integrations setting (a live start/stop, no restart).
    pub(crate) async fn apply(&self, integrations: Integrations) {
        let mut inner = self.inner.lock().await;
        if let Some(server) = inner.mcp.as_mut() {
            server.set(integrations.mcp_enabled).await;
        }
        if let Some(server) = inner.http.as_mut() {
            server.set(integrations.http_api_enabled).await;
        }
    }
}

#[cfg(test)]
#[path = "integration_servers_tests.rs"]
mod tests;
