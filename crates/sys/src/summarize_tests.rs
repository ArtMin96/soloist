use std::path::Path;
use std::time::{Duration, Instant};

use soloist_core::{SummaryError, SummaryInvocation, SummaryRunner};

use super::{run_headless, CommandSummaryRunner};

fn invocation(command_line: &str, stdin: Option<&str>) -> SummaryInvocation {
    SummaryInvocation {
        command_line: command_line.to_string(),
        stdin: stdin.map(str::to_string),
    }
}

/// Runs through a plain POSIX shell so the test is deterministic regardless of the developer's
/// `$SHELL`; the login-shell resolution is covered by the shell-env probe's own tests.
fn run(command_line: &str, stdin: Option<&str>) -> Result<String, SummaryError> {
    run_headless(
        "/bin/sh",
        &invocation(command_line, stdin),
        Duration::from_secs(5),
    )
}

#[test]
fn captures_the_commands_stdout() {
    let out = run("printf 'summary line'", None).expect("command runs");
    assert_eq!(out, "summary line");
}

#[test]
fn runs_in_a_fresh_isolated_working_directory_and_cleans_it_up() {
    // `pwd -P` reports the real cwd: the runner's isolated dir, not the process's own directory —
    // so a summarizer CLI cannot read the app's project context (CLAUDE.md, memory, sessions).
    let dir = run("pwd -P", None).expect("command runs");
    let dir = dir.trim();
    assert!(
        dir.contains("soloist-summarizer-"),
        "ran in the isolated dir, got {dir}"
    );
    // The isolated dir is removed once the run returns — no leak.
    assert!(!Path::new(dir).exists(), "the isolated dir is cleaned up");
}

#[test]
fn feeds_the_prompt_on_stdin() {
    let out = run("cat", Some("piped summary")).expect("command runs");
    assert_eq!(out, "piped summary");
}

#[test]
fn a_nonzero_exit_is_a_failure() {
    assert!(matches!(run("exit 3", None), Err(SummaryError::Failed(_))));
}

#[test]
fn a_hanging_summarizer_times_out_and_is_killed() {
    let started = Instant::now();
    let result = run_headless(
        "/bin/sh",
        &invocation("sleep 5", None),
        Duration::from_millis(200),
    );
    assert!(matches!(result, Err(SummaryError::TimedOut)));
    // The timeout fired well before the 5s sleep would finish — the group was killed and reaped.
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn the_runner_executes_through_the_resolved_login_shell() {
    // Exercises the public path (`login_shell()` → `run_headless`). `contains` tolerates any
    // banner a login profile might print, which the reactor's clamp/trim would drop anyway.
    let out = CommandSummaryRunner::new()
        .run(&invocation("printf 'from the shell'", None))
        .expect("command runs");
    assert!(out.contains("from the shell"));
}
