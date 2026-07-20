use super::{probe_command, DEFAULT_TIMEOUT, PROBE_SCRIPT};

#[test]
fn the_probe_waits_at_least_as_long_as_starting_the_login_shell() {
    // The probe's floor is the login shell's startup, not the CLI's runtime: `-ilc` sources the
    // user's rc files before `--version` runs at all, and a plugin-laden shell takes seconds to
    // reach that point. A budget below what the environment capture allows the very same shell
    // therefore cannot succeed — the deadline fires mid-startup and every CLI is reported
    // absent, on exactly the version-manager setups auto-detection exists to serve. The two
    // budgets pay for the same shell, so this one may not be the smaller.
    assert!(
        DEFAULT_TIMEOUT >= crate::shellenv::DEFAULT_TIMEOUT,
        "the --version probe ({DEFAULT_TIMEOUT:?}) must allow a login shell at least the \
         headroom the environment capture does ({:?})",
        crate::shellenv::DEFAULT_TIMEOUT
    );
}

#[test]
fn the_probe_runs_version_through_an_interactive_login_shell() {
    // Detection must resolve the command against the same login-shell PATH a launch uses —
    // version managers (nvm/asdf/volta) put a CLI on PATH only from interactive rc files — so
    // the probe invokes the interactive login shell (`-ilc`), not a bare `command --version`
    // against the app's inherited PATH.
    let (program, args) = probe_command("/bin/zsh", "claude");
    assert_eq!(program, "/bin/zsh");
    assert_eq!(args[0], "-ilc", "an interactive login shell");
    assert_eq!(args[1], PROBE_SCRIPT, "runs the fixed probe script");
    assert_eq!(
        args[3], "claude",
        "the command is the positional argument $1, never interpolated into the script"
    );
}

#[test]
fn the_probe_passes_the_command_as_a_positional_not_shell_text() {
    // A command carrying shell metacharacters must be bound to $1 verbatim (one program token
    // that simply will not resolve), never spliced into the script where it could be split or
    // injected — the same single-token treatment the spawner gives it.
    let (_program, args) = probe_command("/bin/sh", "claude; rm -rf /");
    assert_eq!(
        args[3], "claude; rm -rf /",
        "the whole command is one positional argument"
    );
    assert!(
        !args[1].contains("rm -rf"),
        "the command never appears in the executed script text"
    );
}
