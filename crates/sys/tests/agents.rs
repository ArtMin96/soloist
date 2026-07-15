//! Auto-detection over the real OS: drives [`CommandVersionProbe`] against temporary
//! executables so the outcome is deterministic and independent of which agent CLIs happen to
//! be installed on the test machine.
//!
//! The probe resolves the command through the user's login shell (`$SHELL -ilc …`, so detection
//! sees the same `PATH` a launched process does). To keep the assertions deterministic and fast
//! — rather than dependent on how heavy the test machine's real shell startup files are — the
//! test points `$SHELL` at a minimal stub shell that ignores the interactive-login flags and just
//! runs the command. The shell-routing itself is unit-tested separately (`probe_command`); here we
//! exercise the real spawn, exit-code reading, and the timeout-guarded kill/reap.
//!
//! The cases run in **one** test so the probes spawn sequentially — matching how the core
//! actually uses the probe (every tool detected one at a time inside a single blocking task)
//! rather than forking several at once, which under load can transiently fail to spawn.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use soloist_core::VersionProbe;
use soloist_sys::CommandVersionProbe;

/// Writes an executable shell script that runs `body`, and returns its absolute path.
fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write script");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod +x");
    path
}

fn path_of(p: &Path) -> &str {
    p.to_str().expect("utf-8 path")
}

#[test]
fn the_version_probe_reflects_whether_a_command_runs() {
    let dir = tempfile::tempdir().expect("temp dir");

    // A minimal stand-in for the login shell: the probe invokes it as
    // `<shell> -ilc <script> <$0> <command>`, so dropping the leading `-ilc` flag and re-running
    // the rest under `/bin/sh -c` runs the script with the same positionals — exercising the real
    // spawn path while staying fast and free of the machine's shell startup files.
    let shell = write_script(
        dir.path(),
        "fake-login-shell",
        "shift\nexec /bin/sh -c \"$@\"",
    );
    std::env::set_var("SHELL", path_of(&shell));

    // A short timeout so the hang case completes promptly against the fast stub shell.
    let probe = CommandVersionProbe::with_timeout(Duration::from_millis(500));

    // A command whose `--version` exits zero is installed.
    let ok = write_script(dir.path(), "agent-ok", "exit 0");
    assert!(
        probe.is_installed(path_of(&ok)),
        "a command whose --version exits 0 is installed"
    );

    // A missing binary is not installed (the shell reports command-not-found → non-zero exit).
    assert!(
        !probe.is_installed("/nonexistent/soloist-not-a-real-agent-binary"),
        "a missing command is not installed"
    );

    // A command whose `--version` exits non-zero is not installed.
    let err = write_script(dir.path(), "agent-err", "exit 3");
    assert!(
        !probe.is_installed(path_of(&err)),
        "a command whose --version fails is not installed"
    );

    // A command that hangs past the timeout is given up on (and reaped), reporting
    // not-installed — and promptly, not after the child's own duration.
    let hang = write_script(dir.path(), "agent-hang", "sleep 10");
    let started = Instant::now();
    assert!(
        !probe.is_installed(path_of(&hang)),
        "a hanging --version times out as not installed"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the probe must time out promptly, not wait for the child"
    );
}
