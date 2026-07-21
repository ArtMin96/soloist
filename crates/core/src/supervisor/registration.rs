//! How a managed process is described to the supervisor before it runs.
//!
//! A [`Registration`] is the input to [`super::Supervisor::register`]: it carries the
//! process's identity, how to launch it, and whether it is trust-gated. The two
//! constructors capture the only two shapes that exist — a trust-gated `solo.yml`
//! command and an ungated terminal or agent the user launches directly.

use std::path::{Path, PathBuf};

use crate::config::ProcessSpec;
use crate::hash::Hash;
use crate::ids::ProjectId;
use crate::ports::{PtySize, SpawnSpec};
use crate::process::ProcessKind;

/// How a registration's label is filed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Labelling {
    /// File the label exactly as given. A `solo.yml` command (whose name is already unique
    /// per project) and an agent (named for its tool) both take their name as-is.
    Exact,
    /// Treat the label as a base name and number it — "Terminal", then "Terminal 2" — against
    /// the labels already in the project, whatever their kind. Resolved inside the registry,
    /// under the same guard that inserts, so two concurrent registrations cannot both claim
    /// the same name.
    NumberedIfTaken,
}

/// How to create a managed process.
pub struct Registration {
    pub project: ProjectId,
    pub kind: ProcessKind,
    pub label: String,
    /// Whether [`label`](Self::label) is the final name or a base to be numbered.
    pub labelling: Labelling,
    pub launch: SpawnSpec,
    /// The project root, recorded as part of the process's orphan-adoption identity.
    pub project_root: PathBuf,
    /// `Some(variant)` makes this a trust-gated command; `None` (terminals and agents,
    /// which the user launches directly) is never trust-gated.
    pub trust_variant: Option<Hash>,
    pub auto_start: bool,
    /// Whether the restart policy relaunches this command after an unexpected exit. From
    /// [`ProcessSpec::auto_restart`]; always `false` for a launched terminal or agent.
    pub auto_restart: bool,
    /// Globs (relative to the project root) whose changes restart this command, from
    /// [`ProcessSpec::restart_when_changed`]; always empty for a launched terminal or agent
    /// (only `solo.yml` commands are file-watched).
    pub restart_when_changed: Vec<String>,
    /// An alternate command line that relaunches this process resuming its last session, when
    /// its provider supports it (an agent's "Resume last session"); `None` otherwise. The
    /// supervisor stores and replays it verbatim, never interpreting it — the per-provider
    /// resume invocation is decided in the agents context and composed by the façade. Its
    /// presence is what makes a process [`resumable`](crate::process::ProcessView::resumable).
    pub resume_command: Option<String>,
}

impl Registration {
    /// A trust-gated [`ProcessKind::Command`] from a `solo.yml` [`ProcessSpec`], with
    /// its working directory resolved against the project root.
    pub fn command(
        project: ProjectId,
        root: &Path,
        name: impl Into<String>,
        spec: &ProcessSpec,
    ) -> Self {
        Self {
            project,
            kind: ProcessKind::Command,
            label: name.into(),
            labelling: Labelling::Exact,
            launch: SpawnSpec {
                command: spec.command.clone(),
                working_dir: spec.resolved_working_dir(root),
                env: spec.env.clone(),
                size: PtySize::default(),
            },
            project_root: root.to_path_buf(),
            trust_variant: Some(spec.variant_hash()),
            auto_start: spec.auto_start,
            auto_restart: spec.auto_restart,
            restart_when_changed: spec.restart_when_changed.clone(),
            // A `solo.yml` command has no agent-style session to resume.
            resume_command: None,
        }
    }

    /// An ungated process (a terminal or agent) launched directly — never trust-gated
    /// and never eligible for auto-start.
    pub fn launched(
        project: ProjectId,
        kind: ProcessKind,
        label: impl Into<String>,
        launch: SpawnSpec,
    ) -> Self {
        // A launched terminal/agent has no project-root command identity; its working
        // directory stands in, so a leftover never matches a configured command.
        let project_root = launch.working_dir.clone();
        Self {
            project,
            kind,
            label: label.into(),
            labelling: Labelling::Exact,
            launch,
            project_root,
            trust_variant: None,
            auto_start: false,
            auto_restart: false,
            restart_when_changed: Vec::new(),
            // Set by [`Self::resumable_with`] for an agent whose provider can resume.
            resume_command: None,
        }
    }

    /// Treats the label as a base name to be numbered against the labels already in the
    /// project ([`Labelling::NumberedIfTaken`]) — how several open terminals stay tellable
    /// apart in the sidebar.
    pub fn numbered(mut self) -> Self {
        self.labelling = Labelling::NumberedIfTaken;
        self
    }

    /// Records the command line that resumes this process's last session, marking it
    /// [`resumable`](crate::process::ProcessView::resumable). Used by the agent launch path for
    /// a provider that supports "Resume last session"; a `None` leaves it non-resumable.
    pub fn resumable_with(mut self, resume_command: Option<String>) -> Self {
        self.resume_command = resume_command;
        self
    }
}
