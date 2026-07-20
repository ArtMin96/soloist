//! Real-OS test for [`CommandShellEnvProbe`]: it runs the actual login shell and reads back
//! a usable environment. Exercises the spawn, the timeout-guarded drain, and the parse
//! against a real shell rather than a fixture.

use std::process::{Command, Stdio};

use soloist_core::ShellEnvProbe;
use soloist_sys::CommandShellEnvProbe;

/// How long to let the precondition shell prove itself before calling this environment
/// shell-less. Comfortably longer than a shell that works needs, short enough that a sandbox
/// without one does not stall the suite.
const PRECONDITION_TIMEOUT_SECS: &str = "5";

/// Whether this machine can run an interactive login shell at all.
///
/// The probe runs `$SHELL -ilc`, and `-i` is the part that does not survive every environment:
/// a container or sandbox with no controlling terminal can leave an interactive shell waiting
/// on input that never comes, which the probe reports as a timeout. That is an absent subject
/// rather than a defect in the code under test, so this asks the question independently — the
/// probe cannot vouch for its own preconditions — and the test skips only when the answer is no.
/// Wherever a shell does work, the assertions below run in full and a probe failure is a failure.
fn interactive_login_shell_works() -> bool {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    Command::new("timeout")
        .args([PRECONDITION_TIMEOUT_SECS, &shell, "-ilc", "printf ok"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn captures_a_real_login_shell_environment_with_path() {
    if !interactive_login_shell_works() {
        eprintln!(
            "skipping: this environment cannot run an interactive login shell, so there is \
             no real shell for the probe to capture"
        );
        return;
    }

    let captured = CommandShellEnvProbe::new()
        .capture()
        .expect("the login shell should export an environment");

    // Every shell exports PATH; it is the whole point of the capture.
    assert!(
        captured.contains_key("PATH"),
        "captured environment is missing PATH: {captured:?}"
    );
    // The capturing shell's own working-directory bookkeeping must be filtered out so it
    // cannot mislead a child process.
    assert!(
        !captured.contains_key("PWD"),
        "session variable PWD leaked into the captured environment"
    );
}
