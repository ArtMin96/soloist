//! Building a [`Spawned`] over an already-running process group.
//!
//! An adopted orphan runs through the *same* actor as a freshly spawned process — we
//! just hand the actor a `Spawned` whose exit future polls the group's liveness, whose
//! control signals the group, whose output is closed (the original PTY died with the
//! previous run — historical output is unrecoverable), and whose I/O is a no-op (there
//! is no live terminal to type into). This avoids a second, parallel actor type.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::ports::{
    Clock, ExitFuture, ExitStatus, OrphanControl, ProcessControl, PtyIo, PtySize, SpawnError,
    Spawned,
};

/// How often an adopted process group's liveness is polled.
const LIVENESS_POLL: Duration = Duration::from_secs(1);

/// Builds a [`Spawned`] over the existing process group `pgid`.
pub(super) fn adopt(pgid: i32, control: Arc<dyn OrphanControl>, clock: Arc<dyn Clock>) -> Spawned {
    let liveness = control.clone();
    let exit: ExitFuture = Box::pin(async move {
        loop {
            clock.sleep(LIVENESS_POLL).await;
            if !liveness.is_alive(pgid) {
                // Died outside our control — no exit code or signal is recoverable.
                return ExitStatus {
                    code: None,
                    signal: None,
                };
            }
        }
    });
    // A closed output channel: there is no live PTY to read from.
    let (_tx, output) = mpsc::channel(1);
    Spawned {
        pid: Some(pgid as u32),
        output,
        exit,
        control: Box::new(GroupSignal { pgid, control }),
        io: Box::new(NoTerminal),
    }
}

/// Signals an adopted process group through the [`OrphanControl`] port.
struct GroupSignal {
    pgid: i32,
    control: Arc<dyn OrphanControl>,
}

#[async_trait]
impl ProcessControl for GroupSignal {
    async fn terminate(&mut self) -> Result<(), SpawnError> {
        self.control.signal(self.pgid, false)
    }

    async fn kill(&mut self) -> Result<(), SpawnError> {
        self.control.signal(self.pgid, true)
    }
}

/// An adopted process has no live terminal; writes and resizes are accepted and dropped.
struct NoTerminal;

#[async_trait]
impl PtyIo for NoTerminal {
    async fn write(&self, _data: &[u8]) -> Result<(), SpawnError> {
        Ok(())
    }

    async fn resize(&self, _size: PtySize) -> Result<(), SpawnError> {
        Ok(())
    }
}
