//! The real [`ProcessSpawner`] adapter.
//!
//! In the walking skeleton this spawns plain OS processes via `tokio::process`; a
//! later phase upgrades it to a full PTY (portable-pty) without changing the port.
//! The invariant that matters now is **process-group containment**: every child is
//! spawned as the leader of a fresh process group, and stop signals target the whole
//! group (via `killpg`), so a process that forks children is torn down completely
//! rather than leaking orphans.

use std::process::Stdio;

use async_trait::async_trait;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::Pid;
use soloist_core::{
    ExitFuture, ExitStatus, ProcessControl, ProcessSpawner, SpawnError, SpawnSpec, Spawned,
};
use tokio::process::Command;

/// Spawns processes with `tokio::process`, placing each in its own process group.
#[derive(Clone, Copy, Default)]
pub struct TokioProcessSpawner;

#[async_trait]
impl ProcessSpawner for TokioProcessSpawner {
    async fn spawn(&self, spec: &SpawnSpec) -> Result<Spawned, SpawnError> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            // `0` makes the child the leader of a new group whose pgid is its pid.
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // Safety net: if the owning task is dropped without a clean stop, the
            // child is killed rather than leaked.
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .map_err(|err| SpawnError::Spawn(err.to_string()))?;

        let pid = child.id();
        let pgid = pid.map(|raw| Pid::from_raw(raw as i32));

        let exit: ExitFuture = Box::pin(async move {
            match child.wait().await {
                Ok(status) => to_exit_status(status),
                // The reaper failed; report an unknown exit rather than panicking.
                Err(_) => ExitStatus {
                    code: None,
                    signal: None,
                },
            }
        });

        Ok(Spawned {
            pid,
            exit,
            control: Box::new(GroupControl { pgid }),
        })
    }
}

/// Signals the child's whole process group. Holds only the pgid, so it never aliases
/// the child handle the exit future owns.
struct GroupControl {
    pgid: Option<Pid>,
}

impl GroupControl {
    fn signal(&self, signal: Signal) -> Result<(), SpawnError> {
        match self.pgid {
            Some(pgid) => killpg(pgid, signal).map_err(|err| SpawnError::Signal(err.to_string())),
            // No pid means the spawn never yielded one; nothing to signal.
            None => Ok(()),
        }
    }
}

#[async_trait]
impl ProcessControl for GroupControl {
    async fn terminate(&mut self) -> Result<(), SpawnError> {
        self.signal(Signal::SIGTERM)
    }

    async fn kill(&mut self) -> Result<(), SpawnError> {
        self.signal(Signal::SIGKILL)
    }
}

fn to_exit_status(status: std::process::ExitStatus) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus {
        code: status.code(),
        signal: status.signal(),
    }
}
