//! Real-OS test for [`CommandShellEnvProbe`]: it runs the actual login shell and reads back
//! a usable environment. Exercises the spawn, the timeout-guarded drain, and the parse
//! against a real shell rather than a fixture.

use soloist_core::ShellEnvProbe;
use soloist_sys::CommandShellEnvProbe;

#[test]
fn captures_a_real_login_shell_environment_with_path() {
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
