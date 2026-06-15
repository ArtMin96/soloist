//! The real [`ProcessSpawner`] adapter, backed by a pseudo-terminal.
//!
//! Each command runs as `$SHELL -lc <command>` on the slave side of a PTY, so the
//! child sees a real terminal (`isatty`) and behaves interactively — colours, cursor
//! control, agent TUIs. Three invariants matter here:
//!
//! * **Login-shell execution** — the shell is resolved from `$SHELL`, then the user's
//!   passwd entry, then `/bin/sh`, and run with `-lc <command>` in the working
//!   directory, with per-process `env` layered onto the inherited environment (process
//!   env wins). `TERM=xterm-256color` is advertised so colour and cursor control work.
//! * **Process-group containment** — `portable-pty` makes the child a session leader,
//!   so its process-group id equals its pid; stop signals target the whole group (via
//!   `killpg`), tearing down a forking command without leaking orphans.
//! * **Bounded, backpressured I/O** — `portable-pty`'s reader, writer, and `wait` are
//!   blocking, so each running process uses two short-lived OS threads: one drains the
//!   master into a bounded channel (blocking when the consumer is slow, so the OS PTY
//!   buffer fills and the child blocks rather than memory growing without limit), and
//!   one reaps the child and resolves its exit future. Both end with the process.

use std::io::{Read, Write};
use std::sync::Mutex;

use async_trait::async_trait;
use nix::errno::Errno;
use nix::libc;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::{Pid, Uid, User};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize as PtPtySize};
use soloist_core::{
    ExitFuture, ExitStatus, OrphanControl, ProcessControl, ProcessSpawner, PtyIo, PtySize,
    SpawnError, SpawnSpec, Spawned,
};
use tokio::sync::{mpsc, oneshot};

/// Fallback shell when neither `$SHELL` nor the passwd entry yields one.
const FALLBACK_SHELL: &str = "/bin/sh";
/// Terminal type advertised to children — a widely supported 256-colour terminfo
/// entry. Soloist ships no custom terminfo.
const TERM: &str = "xterm-256color";
/// Read granularity for the PTY master drain loop.
const READ_CHUNK: usize = 8 * 1024;
/// Bounded depth of the output channel from the read loop to the actor.
const OUTPUT_CAPACITY: usize = 1024;
/// Reported for a signal death whose name the platform does not map to a known number.
const UNKNOWN_SIGNAL: i32 = -1;

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

/// Spawns processes onto a pseudo-terminal, each as the leader of its own process group.
#[derive(Clone, Copy, Default)]
pub struct PtyProcessSpawner;

#[async_trait]
impl ProcessSpawner for PtyProcessSpawner {
    async fn spawn(&self, spec: &SpawnSpec) -> Result<Spawned, SpawnError> {
        let pair = native_pty_system()
            .openpty(to_pt_size(spec.size))
            .map_err(|err| SpawnError::Spawn(err.to_string()))?;

        // `$SHELL -lc <command>`; `CommandBuilder::new` seeds the child env from the
        // current environment, onto which TERM and the per-process overrides layer.
        let mut builder = CommandBuilder::new(login_shell());
        builder.arg("-lc");
        builder.arg(&spec.command);
        builder.cwd(&spec.working_dir);
        builder.env("TERM", TERM);
        for (key, value) in &spec.env {
            builder.env(key, value);
        }

        let mut child = pair
            .slave
            .spawn_command(builder)
            .map_err(|err| SpawnError::Spawn(err.to_string()))?;
        // Drop our copy of the slave so EOF on the master reflects the child closing it.
        drop(pair.slave);

        let pid = child.process_id();
        let pgid = pid.map(|raw| Pid::from_raw(raw as i32));

        // Drain the master into a bounded channel on a blocking thread. It ends on EOF
        // (or read error once the slave closes) or when the actor drops the receiver.
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| SpawnError::Spawn(err.to_string()))?;
        let (output_tx, output) = mpsc::channel::<Vec<u8>>(OUTPUT_CAPACITY);
        std::thread::spawn(move || drain_reader(reader, output_tx));

        // Reap the child on a blocking thread and resolve the exit future once.
        let (exit_tx, exit_rx) = oneshot::channel::<ExitStatus>();
        std::thread::spawn(move || {
            let status = child.wait().map(to_exit_status).unwrap_or(UNKNOWN_EXIT);
            let _ = exit_tx.send(status);
        });
        let exit: ExitFuture = Box::pin(async move { exit_rx.await.unwrap_or(UNKNOWN_EXIT) });

        // The master drives input and resize. `MasterPty` is `Send` but not `Sync` and
        // its writer is blocking, so both live behind mutexes.
        let writer = pair
            .master
            .take_writer()
            .map_err(|err| SpawnError::Spawn(err.to_string()))?;
        let io = Box::new(MasterIo {
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
        });

        Ok(Spawned {
            pid,
            output,
            exit,
            control: Box::new(GroupControl { pgid }),
            io,
        })
    }
}

/// Reported when the reaper itself fails — an unknown exit rather than a panic.
const UNKNOWN_EXIT: ExitStatus = ExitStatus {
    code: None,
    signal: None,
};

/// Reads from the PTY master until EOF or the receiver is gone, forwarding chunks with
/// backpressure. A closed slave surfaces as either a zero-length read or an I/O error
/// depending on the platform; both end the loop.
fn drain_reader(mut reader: Box<dyn Read + Send>, output: mpsc::Sender<Vec<u8>>) {
    let mut buf = [0u8; READ_CHUNK];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if output.blocking_send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

/// Writes input to and resizes a child's PTY through its master.
struct MasterIo {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
}

#[async_trait]
impl PtyIo for MasterIo {
    async fn write(&self, data: &[u8]) -> Result<(), SpawnError> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| SpawnError::Signal("pty writer poisoned".into()))?;
        writer
            .write_all(data)
            .and_then(|()| writer.flush())
            .map_err(|err| SpawnError::Signal(err.to_string()))
    }

    async fn resize(&self, size: PtySize) -> Result<(), SpawnError> {
        let master = self
            .master
            .lock()
            .map_err(|_| SpawnError::Signal("pty master poisoned".into()))?;
        master
            .resize(to_pt_size(size))
            .map_err(|err| SpawnError::Signal(err.to_string()))
    }
}

/// Signals the child's whole process group. Holds only the pgid, so it never aliases
/// the child handle the reaper thread owns.
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

/// Operates on a leftover process group by id for orphan adoption, via `killpg` — the
/// same group-targeting the spawner uses, so a forking orphan is reaped, not orphaned
/// further.
#[derive(Clone, Copy, Default)]
pub struct PgidOrphanControl;

impl OrphanControl for PgidOrphanControl {
    fn is_alive(&self, pgid: i32) -> bool {
        // The null signal performs only existence/permission checks: `Ok` or `EPERM`
        // means the group still has a member; `ESRCH` means it is gone.
        match killpg(Pid::from_raw(pgid), None) {
            Ok(()) | Err(Errno::EPERM) => true,
            Err(_) => false,
        }
    }

    fn signal(&self, pgid: i32, force: bool) -> Result<(), SpawnError> {
        let signal = if force {
            Signal::SIGKILL
        } else {
            Signal::SIGTERM
        };
        killpg(Pid::from_raw(pgid), signal).map_err(|err| SpawnError::Signal(err.to_string()))
    }
}

fn to_pt_size(size: PtySize) -> PtPtySize {
    PtPtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// Maps a `portable-pty` exit status to the core's. A signal death carries no exit
/// code; a normal exit carries its code. The crash/stop classification depends only on
/// success-vs-not (a signal death is never a success), so the recovered signal number
/// is best-effort — `portable-pty` exposes only the platform's signal *description*.
fn to_exit_status(status: portable_pty::ExitStatus) -> ExitStatus {
    match status.signal() {
        Some(name) => ExitStatus {
            code: None,
            signal: Some(signal_number(name)),
        },
        None => ExitStatus {
            code: Some(status.exit_code() as i32),
            signal: None,
        },
    }
}

/// Recovers a signal number from the platform description `portable-pty` reports
/// (`strsignal`). Covers the common signals; an unrecognised description falls back to
/// the `Signal {n}` form, then to a sentinel. The exact number is informational —
/// classification keys off the presence of a signal, not its value.
fn signal_number(name: &str) -> i32 {
    match name {
        "Terminated" => libc::SIGTERM,
        "Killed" => libc::SIGKILL,
        "Interrupt" => libc::SIGINT,
        "Hangup" => libc::SIGHUP,
        "Quit" => libc::SIGQUIT,
        "Aborted" => libc::SIGABRT,
        "Segmentation fault" => libc::SIGSEGV,
        "Broken pipe" => libc::SIGPIPE,
        other => other
            .strip_prefix("Signal ")
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or(UNKNOWN_SIGNAL),
    }
}
