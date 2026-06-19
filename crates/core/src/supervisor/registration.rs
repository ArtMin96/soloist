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

/// How to create a managed process.
pub struct Registration {
    pub project: ProjectId,
    pub kind: ProcessKind,
    pub label: String,
    pub launch: SpawnSpec,
    /// The project root, recorded as part of the process's orphan-adoption identity.
    pub project_root: PathBuf,
    /// `Some(variant)` makes this a trust-gated command; `None` (terminals and agents,
    /// which the user launches directly) is never trust-gated.
    pub trust_variant: Option<Hash>,
    pub auto_start: bool,
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
            launch: SpawnSpec {
                command: spec.command.clone(),
                working_dir: spec.resolved_working_dir(root),
                env: spec.env.clone(),
                size: PtySize::default(),
            },
            project_root: root.to_path_buf(),
            trust_variant: Some(spec.variant_hash()),
            auto_start: spec.auto_start,
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
            launch,
            project_root,
            trust_variant: None,
            auto_start: false,
        }
    }
}
