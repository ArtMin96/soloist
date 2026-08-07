//! A headless one-shot run against real commands, so what is asserted is what the adapter really
//! does with a tool's answer rather than a recording of one. The invocations here stand in for an
//! agent CLI: the adapter was handed a command line by the core and knows nothing else about it.

use std::path::Path;
use std::time::Duration;

use soloist_core::{AgentOneShot, OneShotError, OneShotInvocation};
use soloist_pty::ShellAgentOneShot;

/// Short enough to keep the suite quick; every command here answers at once.
const LIMIT: Duration = Duration::from_secs(30);

fn run(invocation: OneShotInvocation, working_dir: &Path) -> Result<String, OneShotError> {
    ShellAgentOneShot::with_time_limit(LIMIT).run(&invocation, working_dir)
}

fn in_a_directory(invocation: OneShotInvocation) -> Result<String, OneShotError> {
    let dir = tempfile::tempdir().expect("temp dir");
    run(invocation, dir.path())
}

#[test]
fn what_the_tool_printed_is_what_comes_back() {
    let answer = in_a_directory(OneShotInvocation::in_line(
        "printf 'Record the index\\n'".to_string(),
    ))
    .expect("an answer");

    assert_eq!(answer, "Record the index\n");
}

#[test]
fn a_tool_that_reads_its_prompt_is_given_it_on_standard_input() {
    // The convention a user-configured tool may declare. Nothing about the prompt appears in the
    // line, so a subject of any size is handed over without going near a command-line limit.
    let answer = in_a_directory(OneShotInvocation::on_input(
        "cat".to_string(),
        "describe this change",
    ))
    .expect("an answer");

    assert_eq!(answer, "describe this change");
}

#[test]
fn the_run_happens_in_the_project_it_was_asked_about() {
    // An agent CLI reads the project it is standing in — its configuration, its files. Running one
    // anywhere else would describe the wrong repository.
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().canonicalize().expect("canonical root");

    let answer = run(OneShotInvocation::in_line("pwd".to_string()), &root).expect("an answer");

    assert_eq!(answer.trim(), root.to_string_lossy());
}

#[test]
fn a_tool_that_is_not_installed_is_told_apart_from_one_that_failed() {
    // The shell reports both by exit status rather than only in the message it prints, which is what
    // lets a surface say "that tool is not on this machine" without reading translated prose.
    let absent = in_a_directory(OneShotInvocation::in_line(
        "soloist-no-such-agent-tool".to_string(),
    ))
    .expect_err("nothing to run");
    assert_eq!(absent, OneShotError::Missing);

    let failed =
        in_a_directory(OneShotInvocation::in_line("exit 3".to_string())).expect_err("it failed");
    assert_eq!(failed, OneShotError::Failed { status: Some(3) });
}

#[test]
fn a_tool_that_answers_with_far_more_than_a_message_is_refused_rather_than_carried_back() {
    // The reply ceiling, at the one place it decides anything: this tool answers **successfully**,
    // and hands over a quarter of a megabyte. Without the ceiling that comes back as a draft, which
    // is neither something anybody would paste into a message box nor a bound on the app's memory.
    let refusal = in_a_directory(OneShotInvocation::in_line(
        "yes soloist | head -c 250000".to_string(),
    ))
    .expect_err("no usable answer");

    assert!(
        matches!(refusal, OneShotError::Failed { .. }),
        "{refusal:?}",
    );
}
