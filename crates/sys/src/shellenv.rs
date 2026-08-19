//! Capturing the user's login-shell environment: the OS read behind the core's
//! [`ShellEnvProbe`].
//!
//! Runs the user's shell as an interactive login shell and reads back the variables it
//! exports (`$SHELL -ilc 'env -0'`), so a managed process sees the `PATH` a real terminal
//! would — version managers (nvm, rbenv, pyenv) initialised from interactive rc files that
//! a plain `-lc` command shell never sources. Best-effort and bounded: the shell is the one
//! a managed process is launched through ([`soloist_exec::login_shell`]), the capture runs
//! under the shared containment ([`soloist_exec::run`]) — bounded in time and in output,
//! its whole process group stopped and reaped whatever it left running — and the
//! NUL-delimited output is parsed leniently, discarding anything an rc file prints to stdout
//! that is not a variable. The call blocks, so the core runs it off the runtime.

use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;

use soloist_core::{ShellEnvError, ShellEnvProbe};
use soloist_exec::{login_shell, run, Run, RunError};

/// How long to wait for the shell to dump its environment before giving up.
///
/// The ceiling is only meant to guard a hang, so it has to clear a shell that is merely slow. A
/// stock plugin-laden zsh measured 3.7s to reach a prompt on a developer machine — over the 3s
/// this used to allow, so the capture was killed mid-startup and the feature silently degraded to
/// no captured environment on exactly the setups (version managers initialised from interactive rc
/// files) it exists to serve. The capture is best-effort and runs off the async runtime, so waiting
/// longer for an answer costs a background thread its patience, not the user their interface.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// The most one capture may read before it is abandoned.
///
/// An environment is kilobytes, and the kernel caps the whole of a process's arguments and
/// environment at `ARG_MAX` — two megabytes on a stock Linux — so nothing a shell could dump
/// reaches this. It is here for the other thing the pipe can carry: an rc file that writes to
/// standard output and never stops.
const CAPTURE_LIMIT: usize = 2 * 1024 * 1024;

/// The command run inside the login shell: dump the environment NUL-delimited so a value
/// that contains newlines or `=` is parsed unambiguously.
const DUMP_COMMAND: &str = "env -0";

/// Variables the capturing shell sets for its own session; injecting them into a child
/// would be misleading (the child's own shell sets them correctly on startup), so they are
/// dropped from the captured environment.
const SESSION_VARS: [&str; 4] = ["PWD", "OLDPWD", "SHLVL", "_"];

/// Captures the environment of the user's interactive login shell. Stateless; the timeout
/// bounds each capture.
pub struct CommandShellEnvProbe {
    timeout: Duration,
}

impl CommandShellEnvProbe {
    /// A probe with the default capture timeout.
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

impl Default for CommandShellEnvProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellEnvProbe for CommandShellEnvProbe {
    fn capture(&self) -> Result<BTreeMap<String, String>, ShellEnvError> {
        capture_env(&login_shell(), self.timeout)
    }
}

/// Runs `<shell> -ilc 'env -0'` under the shared containment and parses what it wrote. A
/// shell that could not be started, one that did not answer within `timeout` (its process
/// group is stopped and reaped, so nothing it started outlives the call), or output with no
/// recognisable variables is an error, so the resolver falls back to the app environment.
///
/// The exit status is not consulted: an rc file that ends in a non-zero command still
/// exported an environment worth having, and it is the variables that say whether the
/// capture worked.
fn capture_env(shell: &str, timeout: Duration) -> Result<BTreeMap<String, String>, ShellEnvError> {
    let mut command = Command::new(shell);
    command.arg("-ilc").arg(DUMP_COMMAND);

    let finished = run(
        command,
        Run {
            input: None,
            stopped: None,
            time_limit: timeout,
            output_limit: CAPTURE_LIMIT,
            // Whatever an rc file writes about itself is prose, and translated; the variables
            // are the whole answer.
            diagnostics: None,
            // Nobody waits on a capture: it is a background read the user never asked for.
            watching: None,
        },
    )
    .map_err(unanswered)?;

    let env = parse_env0(&finished.output);
    if env.is_empty() {
        return Err(ShellEnvError::Capture(
            "the shell exported no variables".to_string(),
        ));
    }
    Ok(env)
}

/// What a capture that produced nothing has to say for itself. Only a reason to log — the run
/// reports machine data rather than the shell's own wording, so nothing downstream can come to
/// depend on the prose of a program Soloist does not own.
fn unanswered(err: RunError) -> ShellEnvError {
    let reason = match err {
        RunError::Spawn(kind) => format!("the shell could not be started: {kind}"),
        RunError::TimedOut => "the shell did not answer in time".to_string(),
        RunError::OverLimit { .. } => {
            "the shell wrote more than an environment's worth".to_string()
        }
        RunError::Stopped | RunError::Lost => "the capture reached no outcome".to_string(),
    };
    ShellEnvError::Capture(reason)
}

/// Parses NUL-delimited `env -0` output into a variable map, keeping only entries whose
/// name is a valid shell variable name and is not session bookkeeping — so any banner or
/// prompt an interactive rc file writes to stdout is discarded rather than mistaken for a
/// variable.
fn parse_env0(bytes: &[u8]) -> BTreeMap<String, String> {
    String::from_utf8_lossy(bytes)
        .split('\0')
        .filter_map(|entry| {
            let (name, value) = entry.split_once('=')?;
            (is_var_name(name) && !SESSION_VARS.contains(&name))
                .then(|| (name.to_string(), value.to_string()))
        })
        .collect()
}

/// Whether `name` is a POSIX-style environment variable name: a non-empty run of ASCII
/// letters, digits, and underscores that does not start with a digit.
fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
#[path = "shellenv_tests.rs"]
mod tests;
