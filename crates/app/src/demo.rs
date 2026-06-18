//! A built-in demo stack, registered at launch so the dashboard is usable before real
//! project loading lands. This is app-level scaffolding — it routes through the same
//! public façade API real config-driven processes will, and is replaced when
//! "open project → register commands" is wired.
//!
//! It seeds one of each subtype so the grouped sidebar renders end to end: an ungated
//! agent and terminal (interactive shells), and two trust-gated commands that are
//! pre-trusted here so they start without the (deferred) trust dialog.

use std::collections::BTreeMap;
use std::path::PathBuf;

use soloist_core::{Facade, ProcessKind, ProcessSpec, ProjectId, PtySize, Registration, SpawnSpec};

/// The project the demo stack is registered under.
const DEMO_PROJECT: ProjectId = ProjectId::from_raw(1);

/// Registers the demo stack on the supervisor (all `Stopped`; nothing auto-starts until
/// the user acts). Idempotent enough to call once per launch.
pub fn seed(facade: &Facade) {
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    for (kind, label) in [
        (ProcessKind::Agent, "assistant"),
        (ProcessKind::Terminal, "shell"),
    ] {
        facade.supervisor().register(Registration::launched(
            DEMO_PROJECT,
            kind,
            label,
            SpawnSpec {
                command: "bash".into(),
                working_dir: root.clone(),
                env: BTreeMap::new(),
                size: PtySize::default(),
            },
        ));
    }

    for (label, command) in [
        (
            "web",
            "i=0; while true; do echo \"[web] request $i\"; i=$((i+1)); sleep 1; done",
        ),
        (
            "build",
            "echo '[build] compiling'; sleep 2; echo '[build] done'; sleep 1000",
        ),
    ] {
        let spec = command_spec(command);
        // Pre-trust the variant so the demo command is startable without the deferred
        // trust dialog; the real trust flow lands with config-driven loading.
        let _ = facade.trust().trust(DEMO_PROJECT, &spec);
        facade
            .supervisor()
            .register(Registration::command(DEMO_PROJECT, &root, label, &spec));
    }
}

/// A shared command spec for the demo: auto-start eligible (so "Start all" reaches it),
/// no file-watch, no extra env.
fn command_spec(command: &str) -> ProcessSpec {
    ProcessSpec {
        command: command.into(),
        working_dir: None,
        auto_start: true,
        auto_restart: false,
        restart_when_changed: Vec::new(),
        env: BTreeMap::new(),
    }
}
