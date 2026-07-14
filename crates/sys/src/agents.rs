//! Auto-detecting installed agent CLIs: the OS read behind the core's [`VersionProbe`].
//!
//! Runs the provider's `--version` **through the user's login shell** and reports whether it
//! exits successfully — Soloist's signal that the CLI is on this machine. Probing through
//! `$SHELL -ilc` (not a bare `Command`) is what makes detection match reality: a launched
//! process runs with the interactive-login-shell `PATH` (the environment `shellenv` captures),
//! so a CLI installed via a version manager (nvm/asdf/volta) that only edits an interactive rc
//! file is found here exactly as it is at launch. The command is passed to the shell as a
//! **positional argument**, never interpolated into the script text, so it is expanded as one
//! quoted word — the same single program token the spawner launches it as — and a command
//! carrying spaces or shell metacharacters can neither be word-split nor injected. Best-effort
//! and bounded: a missing binary, a non-zero exit, or a hang past the timeout all report
//! not-installed, and a hung probe is killed and reaped so it never leaks. The probe blocks
//! (it spawns and waits on a child), so the core runs it off the async runtime.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use soloist_core::VersionProbe;

use crate::shellenv::login_shell;

/// How long to wait for the login-shell `--version` probe before treating the CLI as
/// unavailable. The probe starts an interactive login shell (to see the launch `PATH`), so it
/// carries the same headroom the environment capture allows a login shell ([`crate::shellenv`]);
/// a real `--version` still returns well within it, and the ceiling only guards a pathological hang.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

/// How often to poll the child while waiting, between spawn and the timeout.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Detects installed agent CLIs by running their `--version`. Stateless; the timeout bounds
/// each probe.
pub struct CommandVersionProbe {
    timeout: Duration,
}

impl CommandVersionProbe {
    /// A probe with the default `--version` timeout.
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// A probe with an explicit timeout (tests use a short one to exercise the hang path
    /// without waiting the full default).
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for CommandVersionProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionProbe for CommandVersionProbe {
    fn is_installed(&self, command: &str) -> bool {
        runs_version_ok(command, self.timeout)
    }
}

/// Whether `command --version`, run through the login shell, exits 0 within `timeout`. The
/// shell not being runnable, the command not being found (a non-zero shell exit), or any
/// non-zero `--version` exit is `false`; a child still running at the deadline is killed,
/// reaped, and reported `false`.
fn runs_version_ok(command: &str, timeout: Duration) -> bool {
    let (program, args) = probe_command(&login_shell(), command);
    let mut child = match Command::new(&program)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        // The shell itself is not runnable — treat the CLI as undetectable.
        Err(_) => return false,
    };

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Past the ceiling: kill and reap so the probe never leaks a process.
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(_) => return false,
        }
    }
}

/// The interactive-login-shell script that runs the probe. The command arrives as `$1` (a
/// positional argument, not interpolated into this text), so the shell expands it as one quoted
/// word — resolved on the login-shell `PATH` exactly like the launched program token — and `exec`
/// replaces the shell with it, so killing the child on timeout reaps the command itself.
const PROBE_SCRIPT: &str = r#"exec "$1" --version"#;

/// Builds the login-shell probe: `<shell> -ilc <PROBE_SCRIPT> soloist-detect <command>`. Under
/// `sh -c`, the operand after the script becomes `$0` and the next becomes `$1`, so the command is
/// bound to `$1` and never parsed as shell text. The interactive-login shell (`-ilc`) sources the
/// rc files where version managers put a CLI on `PATH`, so detection resolves the command against
/// the same `PATH` a launched process sees — mirroring how the environment is captured
/// ([`crate::shellenv`]) and how the spawner runs a command.
fn probe_command(shell: &str, command: &str) -> (String, [String; 4]) {
    (
        shell.to_string(),
        [
            "-ilc".to_string(),
            PROBE_SCRIPT.to_string(),
            "soloist-detect".to_string(),
            command.to_string(),
        ],
    )
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
