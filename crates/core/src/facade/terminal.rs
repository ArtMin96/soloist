//! The terminal surface (context C8 → C2): opening a plain interactive shell in a project.
//!
//! A terminal is the third process subtype, and the only one the user creates with nothing to
//! configure — there is no tool to resolve, no command to compose, and no flags to pass. It is
//! therefore the thinnest possible composition over the supervisor: resolve the project's
//! directory, name the process, register an ungated [`ProcessKind::Terminal`], and start it.
//! Every policy that distinguishes a terminal from a command (no trust gate, no auto-start, no
//! auto-restart, no file-watch restart) already follows from [`Registration::launched`], so
//! none of it is restated here.

use std::collections::{BTreeMap, HashSet};

use super::Facade;
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{PtySize, SpawnSpec, StoreError};
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

/// The label a new terminal takes, before [`next_label`] numbers a duplicate.
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
    /// open terminals stay tellable apart in the sidebar. Many can run at once; each call is a
    /// new process. Must run within a `tokio` runtime (starting spawns the actor).
    pub fn create_terminal(&self, project: ProjectId) -> Result<ProcessId, CreateTerminalError> {
        let root = self
            .projects
            .get(project)?
            .ok_or(CreateTerminalError::UnknownProject)?
            .root;
        let spec = SpawnSpec {
            command: TERMINAL_COMMAND.to_string(),
            working_dir: root,
            // No env overrides: the shell inherits Soloist's environment, layered over the
            // captured login-shell environment exactly like every other spawn.
            env: BTreeMap::new(),
            size: PtySize::default(),
        };
        let id = self.supervisor.register(Registration::launched(
            project,
            ProcessKind::Terminal,
            next_label(TERMINAL_LABEL, &self.labels_in(project)),
            spec,
        ));
        self.supervisor.start(id)?;
        Ok(id)
    }

    /// Every process label in use in `project`, whatever its kind — what a new terminal's label
    /// must not collide with.
    fn labels_in(&self, project: ProjectId) -> HashSet<String> {
        self.supervisor
            .snapshot()
            .into_iter()
            .filter(|view| view.project == project)
            .map(|view| view.label)
            .collect()
    }
}

/// `base` when it is free, otherwise the lowest numbered variant that is ("Terminal 2",
/// "Terminal 3", …).
///
/// Uniqueness spans every kind in the project rather than terminals alone, so a new terminal
/// never renders beside a `solo.yml` command that happens to share its name.
fn next_label(base: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(base) {
        return base.to_string();
    }
    // `taken` is finite and each step tries a distinct label, so at most `taken.len()`
    // candidates can collide and the search always lands.
    let mut suffix = 2;
    let mut candidate = format!("{base} {suffix}");
    while taken.contains(&candidate) {
        suffix += 1;
        candidate = format!("{base} {suffix}");
    }
    candidate
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
