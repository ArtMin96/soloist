//! A headless one-shot run against real commands, so what is asserted is what the adapter really
//! does with a tool's answer rather than a recording of one. The invocations here stand in for an
//! agent CLI: the adapter was handed a command line by the core and knows nothing else about it.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use soloist_core::{AgentOneShot, OneShotError, OneShotInvocation};
use soloist_pty::ShellAgentOneShot;

/// Short enough to keep the suite quick; every command here answers at once. It also bounds a shell
/// that was started in a way that makes it wait for something — so a run that should have answered
/// immediately is reported in seconds rather than left to the real limit.
const LIMIT: Duration = Duration::from_secs(10);

/// The answer every command here gives, and the one line the tool is entitled to have carried back.
const ANSWERED: &str = "Record the index";

fn run(invocation: OneShotInvocation, working_dir: &Path) -> Result<String, OneShotError> {
    run_in(invocation, working_dir, &BTreeMap::new())
}

fn run_in(
    invocation: OneShotInvocation,
    working_dir: &Path,
    env: &BTreeMap<String, String>,
) -> Result<String, OneShotError> {
    ShellAgentOneShot::with_time_limit(LIMIT).run(&invocation, working_dir, env)
}

fn in_a_directory(invocation: OneShotInvocation) -> Result<String, OneShotError> {
    let dir = tempfile::tempdir().expect("temp dir");
    run(invocation, dir.path())
}

/// Prints `ANSWERED`, in the one way every shell in the family builds in.
fn answering() -> OneShotInvocation {
    OneShotInvocation::in_line(format!("printf '{ANSWERED}\\n'"))
}

#[test]
fn what_the_tool_printed_is_what_comes_back() {
    let answer = in_a_directory(answering()).expect("an answer");

    assert_eq!(answer, format!("{ANSWERED}\n"));
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

/// The startup files a shell in this family reads when it is started as a login or an interactive
/// shell. `~/.zshenv` and `$BASH_ENV` are deliberately absent: those are read however a shell is
/// started, so no way of starting one could avoid them, and each shell's own documentation says
/// they must produce no output.
const STARTUP_FILES: [&str; 7] = [
    ".profile",
    ".bash_profile",
    ".bash_login",
    ".bashrc",
    ".zprofile",
    ".zshrc",
    ".zlogin",
];

/// What a startup file prints: a version-manager notice, an update nag, a shell banner.
const BANNER: &str = "Now using node v20.0.0";

/// A home directory of its own, holding a startup file that prints. Disposable, and pointed at by
/// the environment the run is made in, so nothing here reads or writes the real one.
fn home_that_announces_itself() -> (tempfile::TempDir, BTreeMap<String, String>) {
    let home = tempfile::tempdir().expect("temp dir");
    for name in STARTUP_FILES {
        std::fs::write(home.path().join(name), format!("printf '{BANNER}\\n'\n")).expect("write");
    }
    let home_path = home.path().to_string_lossy().into_owned();
    let env = BTreeMap::from([
        ("HOME".to_string(), home_path.clone()),
        // Where zsh looks for its own startup files when it is told to; unset it would fall back to
        // `HOME`, but a developer running the suite may have it pointed elsewhere.
        ("ZDOTDIR".to_string(), home_path),
    ]);
    (home, env)
}

#[test]
fn nothing_the_user_s_startup_files_print_reaches_the_answer() {
    // The whole reason the environment is resolved in the core and handed over: the run needs the
    // user's `PATH`, and the way it used to get it was to ask through their interactive login
    // shell — which reads these files, so whatever one of them printed arrived as part of the
    // drafted commit message. Nothing here is sourced, so what comes back is the tool's own output.
    let (home, env) = home_that_announces_itself();

    let answer = run_in(answering(), home.path(), &env).expect("an answer");

    assert_eq!(answer, format!("{ANSWERED}\n"));
}

#[test]
fn a_tool_only_the_resolved_environment_knows_about_is_still_found() {
    // The reasoning the interactive shell was there for, satisfied properly. This CLI sits where a
    // version manager leaves one: on a `PATH` the app itself never had. Handed that environment the
    // run finds it; without one it is reported absent, which is the same tool "not installed".
    let dir = tempfile::tempdir().expect("temp dir");
    let bin = tempfile::tempdir().expect("temp dir");
    let tool = bin.path().join("soloist-fixture-agent");
    std::fs::write(&tool, format!("#!/bin/sh\nprintf '{ANSWERED}\\n'\n")).expect("write");
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let invocation = OneShotInvocation::in_line("soloist-fixture-agent".to_string());

    let absent = run(invocation.clone(), dir.path()).expect_err("nothing on the app's own path");
    assert_eq!(absent, OneShotError::Missing);

    let env = BTreeMap::from([(
        "PATH".to_string(),
        bin.path().to_string_lossy().into_owned(),
    )]);
    let answer = run_in(invocation, dir.path(), &env).expect("an answer");

    assert_eq!(answer, format!("{ANSWERED}\n"));
}
