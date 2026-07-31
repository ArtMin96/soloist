//! The orchestration context a spawned worker is handed as its first turn.
//!
//! An agent Soloist launches for a lead starts with no memory of why it exists: no skill loaded,
//! no project file read, nothing but its own CLI. So its launch carries an opening turn that
//! says who spawned it, where it is, and what it can reach — composed from the same topic set the
//! `help` tool serves ([`guide`](super::guide)), so a worker is never taught a different Soloist
//! from the one an agent reads about.

use crate::ids::ProcessId;
use crate::projects::ProjectRef;

use super::guide::help_overview;

/// The marker the preamble opens with, so a worker — and a human reading its terminal — can tell
/// the turn Soloist injected from the work itself.
const MARKER: &str = "[SOLO ORCHESTRATION CONTEXT]";

/// The first turn a Soloist-spawned worker is launched with: who spawned it, the project its
/// tools act on, and the capability overview it needs to use the coordination primitives — so it
/// can work without a skill or a project file loaded. `lead` is the process that spawned it, or
/// `None` when the spawning caller was one Soloist could not name.
///
/// The worker's *own* process id is not written in: it is minted by the registration this command
/// line is being built for, so it does not exist yet — and it is already delivered as
/// [`PROCESS_ID_ENV`](crate::ids::PROCESS_ID_ENV) with the worker's MCP session bound to it, which
/// makes `whoami` the authoritative answer rather than a second copy that could disagree.
pub fn orchestration_preamble(project: &ProjectRef, lead: Option<ProcessId>) -> String {
    let spawned_by = match lead {
        Some(lead) => format!("spawned by process #{lead} "),
        None => String::new(),
    };
    let project_name = match &project.name {
        Some(name) => format!("\"{name}\" (#{})", project.id),
        None => format!("#{}", project.id),
    };
    format!(
        "{MARKER}\n\n\
You are a worker agent running under Soloist, {spawned_by}to carry out one piece of work in \
project {project_name}. Soloist injects your own process id into your environment and your MCP \
session is already bound to it, so `whoami` reports who you are and what your tools act on.\n\n\
Coordinate through the shared workspace below rather than ad-hoc files, and leave your result \
where the lead can read it.\n\n\
{}",
        help_overview()
    )
}

#[cfg(test)]
#[path = "preamble_tests.rs"]
mod tests;
