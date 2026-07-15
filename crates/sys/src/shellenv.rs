//! Capturing the user's login-shell environment: the OS read behind the core's
//! [`ShellEnvProbe`].
//!
//! Runs the user's shell as an interactive login shell and reads back the variables it
//! exports (`$SHELL -ilc 'env -0'`), so a managed process sees the `PATH` a real terminal
//! would — version managers (nvm, rbenv, pyenv) initialised from interactive rc files that
//! a plain `-lc` command shell never sources. Best-effort and bounded: the shell is
//! resolved the way the spawner resolves it (`$SHELL`, then the passwd entry, then
//! `/bin/sh`), its output is drained on a thread so a large environment cannot fill the
//! pipe and wedge it, the capture is killed and reaped if it outlives the timeout, and the
//! NUL-delimited output is parsed leniently — discarding anything an rc file prints to
//! stdout that is not a variable. The call blocks, so the core runs it off the runtime.

use std::collections::BTreeMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use nix::unistd::{Uid, User};
use soloist_core::{ShellEnvError, ShellEnvProbe};

/// Fallback shell when neither `$SHELL` nor the passwd entry yields one.
const FALLBACK_SHELL: &str = "/bin/sh";

/// How long to wait for the shell to dump its environment before giving up. An interactive
/// login shell with heavy rc files can take a moment; the ceiling only guards a hang.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

/// How often to poll the shell while waiting, between spawn and the timeout.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

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

/// Resolves the user's login shell: `$SHELL`, then the passwd-entry shell, then `/bin/sh`.
/// The same resolution the PTY spawner uses, so the captured environment is the one the
/// command shell would have. Shared within the crate so `--version` auto-detection probes
/// through the very same shell (and thus the same `PATH`) a launched process runs under.
pub(crate) fn login_shell() -> String {
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

/// Runs `<shell> -ilc 'env -0'`, drains its output on a thread, and parses it. A spawn
/// failure, a hang past `timeout` (the shell is killed and reaped), or output with no
/// recognisable variables is an error, so the resolver falls back to the app environment.
fn capture_env(shell: &str, timeout: Duration) -> Result<BTreeMap<String, String>, ShellEnvError> {
    let mut child = Command::new(shell)
        .arg("-ilc")
        .arg(DUMP_COMMAND)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| ShellEnvError::Capture(format!("spawn {shell}: {err}")))?;

    // Drain stdout on a thread so a large environment cannot fill the pipe and wedge the
    // shell before it exits. On a timeout the kill below closes the pipe, ending this read.
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| ShellEnvError::Capture("no stdout pipe".to_string()))?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(_status)) => break false,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break true;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(ShellEnvError::Capture(format!("wait: {err}")));
            }
        }
    };

    let buf = reader.join().unwrap_or_default();
    if timed_out {
        return Err(ShellEnvError::Capture("timed out".to_string()));
    }
    let env = parse_env0(&buf);
    if env.is_empty() {
        return Err(ShellEnvError::Capture("no variables captured".to_string()));
    }
    Ok(env)
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
