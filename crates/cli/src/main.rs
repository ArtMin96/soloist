//! The `soloist` command-line client: a thin HTTP client of the loopback API. The binary is
//! only an entry point — the surface and behavior live in the [`soloist_cli`] library so they
//! can be exercised by tests.

use std::process::ExitCode;

fn main() -> ExitCode {
    soloist_cli::run()
}
