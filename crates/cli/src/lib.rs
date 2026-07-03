//! The `soloist` CLI: a thin HTTP client of the loopback API. It parses a subcommand,
//! resolves the running app's loopback port from the runtime file the server records, issues
//! **one** request per command to the **same** core method the desktop UI and the MCP server
//! drive, and renders the result. It holds no business logic — name→id resolution and table
//! rendering are the only CLI-side concerns, and even those are pure, testable functions (the
//! CLI is process-isolated from the engine).

mod cli;
mod client;
mod command;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Command};
use client::Client;
use command::Verb;

/// Parses the command line, runs the requested command against the loopback API, and maps the
/// outcome to a process exit code: a one-line success message on stdout, or a one-line
/// `soloist: …` error on stderr with a failing status (so a script can branch on it).
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let client = Client::from_runtime();
    let result = match cli.command {
        Command::Status { status } => command::status(&client, status),
        Command::Start(target) => command::control(&client, Verb::Start, &target),
        Command::Stop(target) => command::control(&client, Verb::Stop, &target),
        Command::Restart(target) => command::control(&client, Verb::Restart, &target),
        Command::Logs { name, lines } => command::logs(&client, &name, lines),
        Command::Focus | Command::Open => command::raise(&client),
        Command::Spawn {
            tool,
            project,
            args,
        } => command::spawn(&client, &tool, &args, project.as_deref()),
        Command::RemoveProject { project } => command::remove_project(&client, &project),
    };
    match result {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("soloist: {err}");
            ExitCode::FAILURE
        }
    }
}
