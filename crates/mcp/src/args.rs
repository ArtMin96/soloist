//! The tool parameter structs. Each derives `schemars::JsonSchema`, which is what rmcp
//! turns into a tool's clean-room input schema, and `Deserialize` to receive the call's
//! arguments. They carry no behaviour — the handlers in [`crate::tools`] destructure them
//! and forward the values to one IPC request.
//!
//! This root holds the supervision and session args; the feature surfaces mirror the
//! [`crate::tools`] taxonomy in submodules, all re-exported flat so a handler names every
//! arg as `crate::args::X`.

mod coordination;
mod git;
mod messaging;
mod prompt_template;
mod setup;

pub(crate) use coordination::*;
pub(crate) use git::*;
pub(crate) use messaging::*;
pub(crate) use prompt_template::*;
pub(crate) use setup::*;

use std::collections::BTreeMap;

use rmcp::schemars;
use serde::Deserialize;

/// Arguments for a single-process tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ProcessArg {
    /// The id of the process, as returned by `list_processes`.
    pub(crate) process: u64,
}

/// Arguments for a project-scoped tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ProjectArg {
    /// The id of the project. Omit to use the session's effective project scope.
    pub(crate) project: Option<u64>,
}

/// Arguments for writing input to a process.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SendInputArg {
    /// The id of the process to write to, as returned by `list_processes`.
    pub(crate) process: u64,
    /// The text to write to the process's input, as UTF-8. Control characters are sent
    /// verbatim — e.g. a trailing carriage return to submit a line, or 0x03 for Ctrl-C.
    pub(crate) input: String,
    /// Optionally wait this many milliseconds after writing, then return the rendered
    /// terminal tail so you can see the effect. Capped by the app; omit to return at once.
    pub(crate) wait_ms: Option<u64>,
}

/// Arguments for renaming a process.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RenameArg {
    /// The id of the process to rename, as returned by `list_processes`.
    pub(crate) process: u64,
    /// The new display label for the process.
    pub(crate) label: String,
}

/// Arguments for spawning a worker agent.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SpawnAgentArg {
    /// The name of a configured agent tool to launch, as listed by `list_agent_tools`.
    pub(crate) tool: String,
    /// Extra command-line flags appended for this one launch ("agent with flags"). Optional.
    #[serde(default)]
    pub(crate) extra_args: Vec<String>,
    /// The first task to queue for the worker after it starts. Omit to spawn without a task.
    pub(crate) prompt: Option<String>,
    /// Whether Soloist should queue its compact agent-coordination instructions for the worker.
    /// Defaults to true; set false only when the worker already has equivalent instructions.
    #[serde(default = "default_true")]
    pub(crate) include_agent_instructions: bool,
}

const fn default_true() -> bool {
    true
}

/// Arguments for spawning a command process.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SpawnProcessArg {
    /// The command line to run. It must **already be trusted** in this project — the user
    /// approves a command's exact command line, working directory, and environment together, and
    /// any combination they have not approved is refused. In practice that means a command from
    /// the project's `solo.yml`.
    pub(crate) command: String,
    /// Where to run it, relative to the project root. Omit to run at the root. Part of what is
    /// trusted, so it must match the approved command exactly.
    pub(crate) working_dir: Option<String>,
    /// Environment overrides for this process. Part of what is trusted, so it must match the
    /// approved command exactly.
    #[serde(default)]
    pub(crate) env: BTreeMap<String, String>,
    /// The display label for the new process. Omit to name it after the command's first word.
    /// A name already used in the project is numbered rather than taken.
    pub(crate) label: Option<String>,
}

/// Arguments for selecting the session's project scope.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SelectProjectArg {
    /// The id of the project to scope this session's tools to, from `list_projects`.
    pub(crate) project: u64,
}

/// Arguments for registering an external caller.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RegisterAgentArg {
    /// A short label identifying the calling agent (e.g. `claude-code`), reported by `whoami`.
    pub(crate) label: String,
}

/// Arguments for reading a process's rendered output.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct OutputArg {
    /// The id of the process, as returned by `list_processes`.
    pub(crate) process: u64,
    /// How many of the most recent rendered lines to return. Omit for the server default;
    /// the app caps it at the rendered scrollback depth.
    pub(crate) lines: Option<usize>,
}

/// Arguments for searching a process's output.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct SearchArg {
    /// The id of the process, as returned by `list_processes`.
    pub(crate) process: u64,
    /// The text to find — a case-sensitive substring. Matching lines are returned in order.
    pub(crate) query: String,
    /// The most matches to return. Omit for the server default; the app caps it.
    pub(crate) limit: Option<usize>,
}

/// Arguments for waiting until a process binds a port.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct WaitForPortArg {
    /// The id of the process to watch, as returned by `list_processes`.
    pub(crate) process: u64,
    /// The localhost port to wait for the process to start listening on.
    pub(crate) port: u16,
    /// How long to wait, in milliseconds. Omit for the server default; the app caps it well
    /// under the request timeout, returning `bound: false` if the port has not bound by then.
    /// While waiting, this call holds the session's connection, so other tool calls on the
    /// same session queue behind it until it returns.
    pub(crate) timeout_ms: Option<u64>,
}
