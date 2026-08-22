//! The exit path's ordering contract: the local integration servers must stop accepting before
//! the supervisor reaps process groups, or a start accepted mid-reap outlives the app. That
//! sequence lives inside a real `App`'s `RunEvent::ExitRequested` handler with no seam to drive it
//! from headlessly, so these drive [`stop_serving_then_reap`] directly against stand-in servers and
//! a stand-in reap instead.

use std::sync::{Arc, Mutex};

use soloist_core::Integrations;
use tokio_util::sync::CancellationToken;

use super::integration_servers::{IntegrationServers, ToggleableServer};
use super::*;

/// Process groups a stand-in server has registered, and the stand-in reap drains — the seam these
/// tests observe real ordering through, instead of asserting a call shape.
type Registry = Arc<Mutex<Vec<u32>>>;

/// A stand-in integration server. On cancellation it registers `group` into `registry` — mirroring
/// how a request already being served when shutdown arrives still runs to its answer (see
/// `handle_connection` in `ipc_server.rs`) before the server's task ends.
fn fake_server(group: u32, registry: Registry) -> ToggleableServer {
    ToggleableServer::new("fake", move |token: CancellationToken| {
        let registry = Arc::clone(&registry);
        async move {
            token.cancelled().await;
            registry.lock().unwrap().push(group);
        }
    })
}

/// A stand-in integration server that never answers its cancellation — standing in for a front
/// parked on something that will never answer, e.g. a `git push` sitting at a credential prompt.
fn wedged_server() -> ToggleableServer {
    ToggleableServer::new("wedged", |_token: CancellationToken| async {
        std::future::pending::<()>().await;
    })
}

/// A stand-in reap: walks `registry` once and drains it, standing in for the supervisor walking
/// and reaping every process group exactly once.
async fn drain(registry: Registry) {
    registry.lock().unwrap().clear();
}

// The ordering: a process group a server registers only while it is being stopped must still be
// reaped, not left running past the servers' shutdown.
#[tokio::test]
async fn the_exit_sequence_leaves_no_process_group_behind() {
    let registry: Registry = Arc::new(Mutex::new(Vec::new()));
    let servers = IntegrationServers::new(
        Some(fake_server(1, Arc::clone(&registry))),
        Some(fake_server(2, Arc::clone(&registry))),
    );
    servers.apply(Integrations::default()).await;

    stop_serving_then_reap(&servers, drain(Arc::clone(&registry))).await;

    assert!(
        registry.lock().unwrap().is_empty(),
        "a process group a server registered while stopping must be reaped, not left behind"
    );
}

// The bound: a server that never answers its cancellation cannot hold the reap hostage — the
// grace elapses and the reap still runs.
#[tokio::test(start_paused = true)]
async fn the_reap_still_runs_when_a_server_never_stops() {
    let registry: Registry = Arc::new(Mutex::new(vec![1]));
    let servers = IntegrationServers::new(Some(wedged_server()), None);
    servers.apply(Integrations::default()).await;

    stop_serving_then_reap(&servers, drain(Arc::clone(&registry))).await;

    assert!(
        registry.lock().unwrap().is_empty(),
        "a server that never stops must not hold the reap hostage"
    );
}
