//! That a `--version` probe nobody answers leaves nothing of itself behind.
//!
//! A binary of its own holding a single test, for two reasons: it points `$SHELL` at a stub, which
//! is a change to this whole process, and it asks what survived a probe — a question another test's
//! children, alive beside it, would answer for.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::thread;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::signal::killpg;
use nix::unistd::Pid;
use soloist_core::{Detection, VersionProbe};
use soloist_sys::CommandVersionProbe;

/// How long the probe's processes live if nothing stops them. Far past everything this test waits
/// for, so anything still alive at the end survived the probe rather than merely outran it.
const UNSTOPPED_LIFE: &str = "30";

/// The probe's limit here. Long enough for the stub to start and record where it was put, short
/// enough to keep the test quick.
const TIME_LIMIT: Duration = Duration::from_millis(500);

/// How long the group is given to empty out once the probe has returned, and how often that is
/// asked. The probe reaps the process it started itself; what *that* started is killed with it and
/// reaped by init, which is a moment later rather than at once.
const EMPTIES_WITHIN: Duration = Duration::from_secs(5);
const CHECK_INTERVAL: Duration = Duration::from_millis(20);

#[test]
fn a_probe_that_reaches_no_answer_leaves_nothing_of_its_process_group_behind() {
    let dir = tempfile::tempdir().expect("temp dir");
    let group_file = dir.path().join("process-group");
    // A stand-in for the login shell that behaves like a CLI which starts a helper and then never
    // answers: it records the process group it was put in, starts something that would outlive it,
    // and hangs. It ignores the flags the probe passes it, because what is under test here is the
    // containment rather than the shell routing.
    let shell = dir.path().join("stub-login-shell");
    fs::write(
        &shell,
        format!(
            "#!/bin/sh\ncut -d' ' -f5 /proc/$$/stat > {group}\n\
             sleep {UNSTOPPED_LIFE} &\nsleep {UNSTOPPED_LIFE}\n",
            group = group_file.display(),
        ),
    )
    .expect("write the stub shell");
    fs::set_permissions(&shell, fs::Permissions::from_mode(0o755)).expect("chmod +x");
    std::env::set_var("SHELL", &shell);

    let detection = CommandVersionProbe::with_timeout(TIME_LIMIT).probe("an-agent-cli");

    assert_eq!(
        detection,
        Detection::Unknown,
        "a probe that never got an answer reached no answer",
    );
    let group = Pid::from_raw(
        fs::read_to_string(&group_file)
            .expect("the stub records the process group it was put in")
            .trim()
            .parse()
            .expect("a process group id"),
    );
    let deadline = Instant::now() + EMPTIES_WITHIN;
    while killpg(group, None).is_ok() && Instant::now() < deadline {
        thread::sleep(CHECK_INTERVAL);
    }
    assert_eq!(
        killpg(group, None),
        Err(Errno::ESRCH),
        "nothing of the probe outlives it: a process of its own still running, a helper it \
         started, or a zombie of either would all still answer a signal sent to that group",
    );
}
