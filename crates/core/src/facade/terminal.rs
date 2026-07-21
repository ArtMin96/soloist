//! Opening a plain interactive shell in a project.
//!
//! A terminal is the third process subtype, and the only one the user creates with nothing to
//! configure — there is no tool to resolve, no command to compose, and no flags to pass. It is
//! therefore the thinnest possible composition over the supervisor: resolve the project's
//! directory, register an ungated [`ProcessKind::Terminal`] under a numbered label, and start
//! it. Every policy that distinguishes a terminal from a command (no trust gate, no auto-start,
//! no auto-restart, no file-watch restart) already follows from [`Registration::launched`], so
//! none of it is restated here.

use super::Facade;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{SpawnSpec, StoreError};
use crate::process::ProcessKind;
use crate::supervisor::{Registration, SupervisorError};

/// The command a new terminal runs.
///
/// The spawner already invokes every command through the user's login shell, resolving it as
/// `$SHELL`, then the passwd entry, then `/bin/sh`. Re-execing that shell is what turns the
/// wrapper into an interactive session, and it keeps the resolution in the one adapter that can
/// see the machine — the core owns no view of which shells exist. `exec` replaces the wrapper
/// rather than nesting inside it, so the process group the supervisor signals is the shell the
/// user is typing at. The `:-` fallback covers a session that exports no `SHELL` (a desktop
/// launcher does not always), which would otherwise expand to an empty command line and exit
/// immediately.
const TERMINAL_COMMAND: &str = "exec ${SHELL:-/bin/sh}";

/// The base label a new terminal takes; the registry numbers a duplicate ("Terminal 2", …).
const TERMINAL_LABEL: &str = "Terminal";

impl Facade {
    /// Opens a plain interactive shell in `project`'s directory as an ungated
    /// [`ProcessKind::Terminal`] process and starts it, returning its process id.
    ///
    /// The shell is not configurable and takes no caller-supplied command: a terminal is the
    /// user's own shell, and the local user already has one. That keeps the only thing this
    /// spawns a fixed constant, so no surface can turn it into arbitrary code execution.
    ///
    /// The label is unique within the project ("Terminal", then "Terminal 2", …) so several
    /// open terminals stay tellable apart in the sidebar; the registry resolves it as it
    /// files the process. Many can run at once; each call is a new process. Must run within a
    /// `tokio` runtime (starting spawns the actor).
    pub fn create_terminal(&self, project: ProjectId) -> Result<ProcessId, CreateTerminalError> {
        let root = self
            .project_root(project)?
            .ok_or(CreateTerminalError::UnknownProject)?;
        let id = self.supervisor.register(
            Registration::launched(
                project,
                ProcessKind::Terminal,
                TERMINAL_LABEL,
                SpawnSpec::inheriting_env(TERMINAL_COMMAND, root),
            )
            .numbered(),
        );
        self.supervisor.start(id)?;
        Ok(id)
    }
}

/// Why opening a terminal failed: the project is not known, a durable read failed, or the
/// supervisor refused to start the process.
#[derive(Debug, thiserror::Error)]
pub enum CreateTerminalError {
    #[error("no such project")]
    UnknownProject,
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
