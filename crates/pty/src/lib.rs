//! The real [`ProcessSpawner`] adapter.
//!
//! In the walking skeleton this spawns plain OS processes via `tokio::process`; a
//! later phase upgrades it to a full PTY (portable-pty) without changing the port.
//! Two invariants matter here:
//!
//! * **Login-shell execution** — every command runs as `$SHELL -lc <command>` in its
//!   resolved working directory, with per-process `env` layered onto the inherited
//!   environment (process env wins). This is how aliases and version-manager PATHs
//!   resolve. The shell is resolved from `$SHELL`, then the user's passwd entry, then
//!   `/bin/sh`, so a desktop launch that does not export `$SHELL` still uses the user's
//!   real shell rather than a bare `/bin/sh`. Full `$SHELL -ilc env` capture/caching is
//!   a later phase.
//! * **Process-group containment** — each child is the leader of a fresh process
//!   group, and stop signals target the whole group (via `killpg`), so a command that
//!   forks children is torn down completely rather than leaking orphans.

use std::process::Stdio;

use async_trait::async_trait;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::{Pid, Uid, User};
use soloist_core::{
    ExitFuture, ExitStatus, ProcessControl, ProcessSpawner, SpawnError, SpawnSpec, Spawned,
};
use tokio::process::Command;

/// Fallback shell when neither `$SHELL` nor the passwd entry yields one.
const FALLBACK_SHELL: &str = "/bin/sh";

/// Resolves the user's login shell: `$SHELL`, then the passwd-entry shell, then
/// `/bin/sh`. A desktop launcher does not always export `$SHELL`, so the passwd
/// fallback keeps commands running under the user's real shell.
fn login_shell() -> String {
    if let Ok(shell) = std::env::var("SHELL") {
        if !shell.is_empty() {
            return shell;
        }
    }
    if let Ok(Some(user)) = User::from_uid(Uid::current()) {
        if let Some(shell) = user.shell.to_str() {
            if !shell.is_empty() {
                return shell.to_owned();
            }
        }
    }
    FALLBACK_SHELL.to_string()
}

/// Spawns processes with `tokio::process`, placing each in its own process group.
#[derive(Clone, Copy, Default)]
pub struct TokioProcessSpawner;

#[async_trait]
impl ProcessSpawner for TokioProcessSpawner {
    async fn spawn(&self, spec: &SpawnSpec) -> Result<Spawned, SpawnError> {
        let mut command = Command::new(login_shell());
        command
            .arg("-lc")
            .arg(&spec.command)
            .current_dir(&spec.working_dir)
            // Per-process overrides layer onto the inherited app env (process wins).
            .envs(&spec.env)
            // `0` makes the child the leader of a new group whose pgid is its pid.
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // Safety net: if the owning task is dropped without a clean stop, the child
            // is killed rather than leaked.
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
