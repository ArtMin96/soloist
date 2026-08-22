//! That a login-shell capture answers even when run from a process that has a real controlling
//! terminal — reproducing the regression directly, rather than only the property that closes it.
//!
//! Before the fix, a captured shell was left in the *caller's* session: it got a new process
//! group but inherited whatever controlling terminal that session had, and that new group was
//! never the terminal's foreground group. An interactive shell's own job-control startup calls
//! `tcsetpgrp()` on that terminal regardless of what its own stdio is wired to, and a
//! background-group process doing that is stopped by `SIGTTOU` — which nothing here ever
//! releases it from, since nothing makes it the foreground group again. The fix gives the child a
//! session of its own with no controlling terminal at all, so that call has nothing to reach.
//!
//! No `#[test]`: giving *this* process a controlling terminal is `setsid()` followed by opening
//! the terminal device by path, and `setsid()` is a per-*thread* kernel operation on Linux — it
//! seats the session on the calling kernel task, not on the process's thread-group id, so calling
//! it from one of the libtest harness's worker threads (where every `#[test]` body runs) does not
//! reliably make it *this process's* session in the sense a forked child, or the process's own
//! exit handling, understands. Cargo runs this target as `harness = false`, so `main` runs on the
//! process's real main thread, the one every other Soloist process making this call is on too.
//!
//! Every exit here goes through [`std::process::exit`], never a return from `main`: once the pty
//! is acquired as this process's controlling terminal, dropping its master side hangs up the
//! slave, and this process — the terminal's own foreground group — would be sent that hangup
//! itself. `process::exit` tears every file descriptor down together as the kernel exits the
//! process, rather than one at a time while it is still running to receive what closing one of
//! them causes.

use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};

use nix::pty::openpty;
use nix::unistd::setsid;

use soloist_core::ShellEnvProbe;
use soloist_sys::CommandShellEnvProbe;

/// How long the capture may take before it is given up on. Comfortably past what a real shell
/// needs, short enough that a regression — the shell `SIGTTOU`-stopped, which nothing here ever
/// releases it from — is caught in seconds rather than left to the suite's own timeout.
const TIME_LIMIT: Duration = Duration::from_secs(3);

/// The longest a capture that actually ran may take. A plain bash startup is tens of
/// milliseconds; this stays far below [`TIME_LIMIT`] so a capture that merely answered late is
/// told apart from one that hung until the limit stopped it.
const ANSWERS_WITHIN: Duration = Duration::from_secs(1);

/// The process's exit code convention: `0` for the observable outcome the fix promises, `1` for
/// anything else. `cargo test` reads any other exit — including a raw signal — as a failure too,
/// so both are written to be that anyway; this is the deliberate one.
const FAILURE: i32 = 1;

fn main() {
    // A real login shell is required: job-control setup — the code path that calls
    // `tcsetpgrp()` and can be `SIGTTOU`-stopped — is a feature of an interactive shell, not of
    // the POSIX `sh` this crate's other tests stub in.
    std::env::set_var("SHELL", "/bin/bash");

    let pty = openpty(None, None).expect("open a pseudo-terminal");
    // What gives a session a controlling terminal is *opening* the device by name while the
    // session has none — not merely holding an fd already open on it, which is all the master
    // side, opened before this process has a session of its own, does. So only the path is kept.
    let slave_path = std::fs::read_link(format!("/proc/self/fd/{}", pty.slave.as_raw_fd()))
        .expect("resolve the pseudo-terminal slave's device path");
    drop(pty.slave);

    // This process becomes the leader of a new session with no controlling terminal — the same
    // state a shell launched from a real terminal is in relative to *that* terminal, which is
    // what every run this crate starts inherits today.
    setsid().expect("this process is not already a process-group leader");
    // Opening the slave now, as that session's leader, makes it this session's controlling
    // terminal. Kept open, alongside the master, for the rest of the run — see the module doc for
    // why neither is ever allowed to drop before `process::exit`.
    let _ctty = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&slave_path)
        .expect("open the pseudo-terminal slave as this session's controlling terminal");
    let _master = pty.master;

    let started = Instant::now();
    let captured = CommandShellEnvProbe::with_timeout(TIME_LIMIT).capture();
    let took = started.elapsed();

    let outcome = match captured {
        Err(err) => Err(format!(
            "a login-shell capture has to answer even from a session with a controlling \
             terminal, not be silently stopped by it: {err}"
        )),
        Ok(env) if !env.contains_key("PATH") => Err(format!(
            "a shell that truly ran exports PATH; a capture that merely avoided the hang \
             without the shell finishing would not have it: {env:?}"
        )),
        Ok(_) if took >= ANSWERS_WITHIN => Err(format!(
            "a shell that ran normally answers in well under its time limit; taking this long \
             says it was stopped and only released at the limit: took {took:?}"
        )),
        Ok(_) => Ok(()),
    };

    match outcome {
        Ok(()) => std::process::exit(0),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(FAILURE);
        }
    }
}
