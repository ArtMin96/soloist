//! Shared registration fixtures for driving the supervisor thread end to end, reused by
//! the core's own tests and, via the `testing` feature, by adapter-crate integration
//! tests.

use std::collections::BTreeMap;

use crate::ids::ProjectId;
use crate::ports::{PtySize, SpawnSpec};
use crate::process::ProcessKind;
use crate::supervisor::Registration;

/// Builds a [`Registration`] for an ungated terminal running `command` — the minimal
/// launched-process fixture for exercising the supervisor thread (register → start →
/// stop) end to end across crates. A terminal is ungated, so the trust gate is never
/// consulted; the working directory is `.` (irrelevant to a self-contained command).
pub fn terminal_registration(project: ProjectId, name: &str, command: &str) -> Registration {
    Registration::launched(
        project,
        ProcessKind::Terminal,
        name,
        SpawnSpec {
            command: command.into(),
            working_dir: ".".into(),
            env: BTreeMap::new(),
            size: PtySize::default(),
        },
    )
}
