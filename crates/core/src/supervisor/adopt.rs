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
    Clock, ExitFuture, ExitStatus, OrphanControl, OrphanRecord, ProcessControl, PtyIo, PtySize,
    SpawnError, Spawned,
};

/// How often an adopted process group's liveness is polled.
const LIVENESS_POLL: Duration = Duration::from_secs(1);

/// Builds a [`Spawned`] over the existing process group recorded in `record`. The poll
/// verifies the recorded *identity*, not a bare pgid, so if the adopted group dies and the
/// OS reassigns its pgid to an unrelated group, the poll still observes the death instead
/// of mistaking the impostor for the adopted process (and later signalling it on stop).
pub(super) fn adopt(
    record: OrphanRecord,
    control: Arc<dyn OrphanControl>,
    clock: Arc<dyn Clock>,
) -> Spawned {
    let pgid = record.pgid;
    let signal_record = record.clone();
    let liveness = control.clone();
    let exit: ExitFuture = Box::pin(async move {
        loop {
            clock.sleep(LIVENESS_POLL).await;
            if !liveness.is_recorded_alive(&record) {
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
        control: Box::new(GroupSignal {
            record: signal_record,
            control,
        }),
        io: Box::new(NoTerminal),
    }
}

/// Signals an adopted process group through the [`OrphanControl`] port — but only while
/// the recorded identity still matches, so a stop or kill can never signal a pgid the OS
/// reassigned to an unrelated group after the adopted process died.
struct GroupSignal {
    record: OrphanRecord,
    control: Arc<dyn OrphanControl>,
}

#[async_trait]
impl ProcessControl for GroupSignal {
    async fn terminate(&mut self) -> Result<(), SpawnError> {
        if self.control.is_recorded_alive(&self.record) {
            self.control.signal(self.record.pgid, false)
        } else {
            // Gone or recycled — nothing of ours to signal.
            Ok(())
        }
    }

    async fn kill(&mut self) -> Result<(), SpawnError> {
        if self.control.is_recorded_alive(&self.record) {
            self.control.signal(self.record.pgid, true)
        } else {
            // Gone or recycled — nothing of ours to signal.
            Ok(())
        }
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

#[cfg(test)]
#[path = "adopt_tests.rs"]
mod tests;
