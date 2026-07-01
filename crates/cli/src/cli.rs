//! The `soloist` command-line surface: the subcommands and their arguments, defined with
//! clap's derive API. This module is parsing only — every command's behavior lives in
//! [`crate::command`].

use clap::{Args, Parser, Subcommand, ValueEnum};

/// `soloist` — control the local Soloist stack from a shell, over its loopback HTTP API.
#[derive(Debug, Parser)]
#[command(
    name = "soloist",
    version,
    about = "Control the local Soloist stack over its loopback HTTP API."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// The CLI's verbs. Each maps to one HTTP endpoint, which maps to one core command.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// List the processes in the running stack as a table.
    Status {
        /// Show only processes in this status.
        #[arg(long, value_enum)]
        status: Option<StatusFilter>,
    },
    /// Start a process by name, or every command in a project with `all`.
    Start(Target),
    /// Stop a process by name, or every process in a project with `all`.
    Stop(Target),
    /// Restart a process by name, or every command in a project with `all`.
    Restart(Target),
    /// Print a process's recent output.
    Logs {
        /// The process whose output to print.
        name: String,
        /// How many of the most recent lines to print (default: the API's recent window).
        #[arg(short = 'n', long)]
        lines: Option<usize>,
    },
    /// Raise the Soloist window to the front.
    Focus,
    /// Open Soloist — raise its window to the front (an alias of `focus`).
    Open,
}

/// What `start`/`stop`/`restart` act on: a named process, or `all` for a whole project. Shared
/// by the three verbs so their argument shape is defined once.
#[derive(Debug, Args)]
pub struct Target {
    /// A process name, or `all` to act on every command in the project.
    pub target: String,
    /// When the target is `all` and more than one project is open, the project to act on.
    #[arg(long)]
    pub project: Option<String>,
}

/// The statuses `status --status` can filter to — the two a shell most often watches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StatusFilter {
    Running,
    Crashed,
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
