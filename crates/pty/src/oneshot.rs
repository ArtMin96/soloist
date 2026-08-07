//! The real [`AgentOneShot`] adapter: one configured agent tool, run once, through the user's shell.
//!
//! It is the deliberate opposite of everything else in this crate. No pseudo-terminal is opened, so
//! the tool sees no terminal and runs whatever non-interactive path it has; nothing is attached to
//! it and no output is streamed anywhere; and it either answers within a bounded time or is stopped
//! and reaped. The containment is [`soloist_exec`], the same discipline every external tool in the
//! app runs under.
//!
//! Which invocation to make, and which environment to make it in, were both decided in the core —
//! this runs the line it was handed and reads back what was written, so no provider knowledge lives
//! here.
//!
//! **The shell is here to read the line, and for nothing else.** A command line arrives POSIX-quoted
//! as one string, so something has to parse it; the `PATH` that finds a CLI a version manager
//! installed arrives in the environment instead. So the shell is started neither as a login shell
//! nor as an interactive one, because either would read the user's startup files and **whatever
//! those print to standard output would arrive as part of the answer** — a shell banner or an update
//! notice inside a commit message. Dropping only the interactive flag would not be enough:
//! `bash(1)` reads the same `/etc/profile` and `~/.bash_profile` set "as a non-interactive shell
//! with the `--login` option" as it does when interactive, and `zsh(1)` reads `.zprofile` and
//! `.zlogin` for any login shell. What remains is the one file a shell reads however it is started
//! (`~/.zshenv` for zsh, `$BASH_ENV` for bash), which the shell's own documentation says must not
//! produce output.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use soloist_core::{AgentOneShot, OneShotError, OneShotInvocation, ONE_SHOT_REPLY_LIMIT};
use soloist_exec::{Run, RunError};

use crate::login_shell;

/// How long one run may take before it is stopped.
///
/// Its floor is not the tool's: an agent CLI pays the user's shell startup, then a model's latency,
/// and it is answering a question about a whole staged change. Generous enough that a real answer
/// arrives, bounded so a tool waiting on something that will never come — a credential prompt it
/// cannot show, a network that is gone — cannot hold the blocking thread for ever.
const TIME_LIMIT: Duration = Duration::from_secs(120);

/// The exit status a POSIX shell reports for a command it could not find, and for one it found and
/// could not execute. Both mean the tool is not runnable on this machine, which is a different thing
/// from the tool running and failing — and unlike the message the shell prints alongside them, they
/// are numbers rather than translated prose.
const NOT_FOUND: i32 = 127;
const NOT_EXECUTABLE: i32 = 126;

/// Runs one agent tool headless through the user's shell, bounded.
///
/// Stateless apart from its time limit, which a test shortens to exercise the stopping path without
/// waiting out the real one.
#[derive(Clone, Copy)]
pub struct ShellAgentOneShot {
    time_limit: Duration,
}

impl ShellAgentOneShot {
    /// A runner with the default time limit.
    pub fn new() -> Self {
        Self {
            time_limit: TIME_LIMIT,
        }
    }

    /// A runner with an explicit time limit.
    pub fn with_time_limit(time_limit: Duration) -> Self {
        Self { time_limit }
    }
}

impl Default for ShellAgentOneShot {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentOneShot for ShellAgentOneShot {
    fn run(
        &self,
        invocation: &OneShotInvocation,
        working_dir: &Path,
        env: &BTreeMap<String, String>,
    ) -> Result<String, OneShotError> {
        let mut command = Command::new(login_shell());
        command
            .arg("-c")
            .arg(&invocation.command_line)
            .current_dir(working_dir)
            // The environment a managed process is launched with, layered onto the app's own the way
            // the spawner layers it. It carries the `PATH` an interactive login shell would have, so
            // the tool the user was told is installed is the tool that runs.
            .envs(env);

        let finished = soloist_exec::run(
            command,
            Run {
                stopped: None,
                input: invocation.input.as_deref(),
                time_limit: self.time_limit,
                output_limit: ONE_SHOT_REPLY_LIMIT,
                // Whatever it wrote about itself is prose, and translated. Only the exit status
                // crosses back, which is what keeps this adapter's behaviour independent of the
                // wording of a program Soloist does not own.
                diagnostics: None,
            },
        )
        .map_err(failure)?;

        if finished.status.success() {
            return Ok(String::from_utf8_lossy(&finished.output).into_owned());
        }
        Err(match finished.status.code() {
            Some(NOT_FOUND) | Some(NOT_EXECUTABLE) => OneShotError::Missing,
            status => OneShotError::Failed { status },
        })
    }
}

/// What a run that never reached an end means for a draft.
fn failure(err: RunError) -> OneShotError {
    match err {
        // Nothing here asks a one-shot run to stop, so it can only end the other ways.
        RunError::TimedOut | RunError::Stopped => OneShotError::Timeout,
        // The shell itself could not be started, the wait could not report, or the tool would not
        // stop writing. None is the tool being absent, and none of them produced an answer.
        RunError::Spawn(_) | RunError::Lost => OneShotError::Failed { status: None },
        RunError::OverLimit { status } => OneShotError::Failed { status },
    }
}
