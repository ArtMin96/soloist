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
//! and bounded: a missing binary or a non-zero exit reports not-installed, a hang past the
//! timeout or an unrunnable shell reports "no answer" rather than a false absence, and a hung
//! probe is killed and reaped so it never leaks. The probe blocks (it spawns and waits on a
//! child), so the core runs it off the async runtime.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use soloist_core::{Detection, VersionProbe};

use crate::shellenv::login_shell;

/// How long to wait for the login-shell `--version` probe before giving up on an answer.
///
/// The probe starts an interactive login shell (to see the launch `PATH`), so its floor is that
/// shell's startup, not the CLI's. A plugin-laden zsh takes ~4s just to reach the point of
/// running anything — which is why this matches, rather than undercuts, the headroom the
/// environment capture allows a login shell ([`crate::shellenv`]). Budget the probe below shell
/// startup and every CLI is reported absent on exactly the setups auto-detection exists to
/// serve. A real `--version` returns well within this, and the ceiling only guards a
/// pathological hang.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

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
    fn probe(&self, command: &str) -> Detection {
        probe_version(command, self.timeout)
    }
}

/// What `command --version`, run through the login shell, reveals within `timeout`.
///
/// A clean exit is [`Detection::Installed`]; the command not being found (the shell exits
/// non-zero) or any failing `--version` is [`Detection::Missing`] — both are answers about the
/// machine. Not being able to run the shell at all, or a child still running at the deadline
/// (killed and reaped so the probe never leaks a process), is [`Detection::Unknown`]: the probe
/// reached no answer, which is not the same as the CLI being absent.
fn probe_version(command: &str, timeout: Duration) -> Detection {
    let (program, args) = probe_command(&login_shell(), command);
    let mut child = match Command::new(&program)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        // The shell itself is not runnable, so nothing was learned about the CLI.
        Err(_) => return Detection::Unknown,
    };

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Detection::Installed
                } else {
                    Detection::Missing
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Past the ceiling: kill and reap so the probe never leaks a process.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Detection::Unknown;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(_) => return Detection::Unknown,
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
