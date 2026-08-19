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
//! timeout or an unrunnable shell reports "no answer" rather than a false absence, and a probe
//! runs under the shared containment ([`soloist_exec::run`]), so its whole process group is
//! stopped and reaped whatever it left running. The probe blocks (it spawns and waits on a
//! child), so the core runs it off the async runtime.

use std::process::Command;
use std::time::Duration;

use soloist_core::{Detection, VersionProbe};
use soloist_exec::{login_shell, run, Run, RunError};

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

/// The most a `--version` may print before the probe stops reading it.
///
/// Nothing here reads what was printed — the exit status is the whole answer — so this is not a
/// budget for a version string but a ceiling on a command that mistakes `--version` for an
/// invitation to print for ever. A version line is tens of bytes; this is orders of magnitude
/// above every real one.
const ANSWER_LIMIT: usize = 64 * 1024;

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
/// A clean exit, or one that outran the output ceiling, is [`Detection::Installed`]; the command
/// not being found (the shell exits non-zero) or any failing `--version` is [`Detection::Missing`]
/// — all of them answers about the machine. Not being able to run the shell at all, or a run that
/// reached no outcome within the timeout (its process group is stopped and reaped, so the probe
/// leaves nothing behind), is [`Detection::Unknown`]: the probe reached no answer, which is not the
/// same as the CLI being absent.
fn probe_version(command: &str, timeout: Duration) -> Detection {
    let (program, args) = probe_command(&login_shell(), command);
    let mut probe = Command::new(program);
    probe.args(args);

    match run(
        probe,
        Run {
            input: None,
            stopped: None,
            time_limit: timeout,
            output_limit: ANSWER_LIMIT,
            // A `--version` is answered by its exit status; whatever it printed, on either
            // stream, says nothing about whether it is installed.
            diagnostics: None,
            // Nobody waits on a detection: it answers in milliseconds and reports one word.
            watching: None,
        },
    ) {
        Ok(finished) if finished.status.success() => Detection::Installed,
        Ok(_) => Detection::Missing,
        // A `--version` that outran the ceiling demonstrably ran, so calling it absent would be
        // exactly the false absence this probe is built to avoid.
        Err(RunError::OverLimit { .. }) => Detection::Installed,
        Err(RunError::TimedOut | RunError::Stopped | RunError::Lost | RunError::Spawn(_)) => {
            Detection::Unknown
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
