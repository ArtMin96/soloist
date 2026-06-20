//! Port readiness: wait until a process binds an expected port.
//!
//! Polls the [`PortProbe`] on the [`Clock`] until the awaited port appears among the
//! process group's listening ports, or the timeout elapses. While waiting, the process's
//! readiness gate reads [`Readiness::Waiting`] (Running but not Ready); on bind it becomes
//! [`Readiness::Ready`]. The OS read is the portscan adapter's; the timing is core policy
//! (mock-clock testable). The Facade exposes this as `wait_for_port`; the MCP
//! `wait_for_bound_port` tool is the production caller once it lands.

use std::sync::Arc;
use std::time::Duration;

use crate::ids::ProcessId;
use crate::ports::Clock;
use crate::supervision::run_blocking;
use crate::supervisor::Supervisor;

use super::PortProbe;

/// How often the awaited port is re-checked while waiting.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Why a [`wait_for_port`] did not resolve.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WaitForPortError {
    /// The process has no live group to probe — it is not running (or stopped while waiting).
    #[error("process is not running")]
    NotRunning,
    /// The port did not bind within the timeout.
    #[error("timed out waiting for the port to bind")]
    Timeout,
}

/// Resolves once process `id` is listening on `port`, or errors on timeout. While waiting,
/// the process reads as Running-but-not-Ready ([`crate::process::ProcessView::ready`] =
/// `Some(false)`), flipping to `Some(true)` on bind. Takes owned handles so it can run as
/// its own task; re-resolves the group each poll, so a process that stops mid-wait fails
/// fast with [`WaitForPortError::NotRunning`].
pub async fn wait_for_port(
    supervisor: Arc<Supervisor>,
    probe: Arc<dyn PortProbe>,
    clock: Arc<dyn Clock>,
    id: ProcessId,
    port: u16,
    timeout: Duration,
) -> Result<(), WaitForPortError> {
    let deadline = clock.now() + timeout;
    let mut announced_waiting = false;
    loop {
        let Some(pgid) = supervisor.pgid_of(id) else {
            return Err(WaitForPortError::NotRunning);
        };
        if is_bound(probe.clone(), pgid, port).await {
            supervisor.set_ready(id, pgid, true);
            return Ok(());
        }
        if !announced_waiting {
            // Mark Running-but-not-Ready only once we know it isn't already bound, so an
            // already-listening process never flickers through "not ready".
            supervisor.set_ready(id, pgid, false);
            announced_waiting = true;
        }
        if clock.now() >= deadline {
            return Err(WaitForPortError::Timeout);
        }
        clock.sleep(POLL_INTERVAL).await;
    }
}

/// Whether `pgid`'s group currently lists `port` among its listening ports. The `/proc`
/// read runs on the blocking pool so a poll never stalls a runtime worker.
async fn is_bound(probe: Arc<dyn PortProbe>, pgid: i32, port: u16) -> bool {
    run_blocking(move || {
        probe
            .listening_ports(&[pgid])
            .get(&pgid)
            .is_some_and(|ports| ports.contains(&port))
    })
    .await
}

#[cfg(test)]
#[path = "waiter_tests.rs"]
mod tests;
