//! That an environment capture ends within its own time limit even when the shell leaves something
//! behind holding the pipe the capture reads.
//!
//! A binary of its own holding a single test: it points `$SHELL` at a stub, which is a change to
//! this whole process, so a capture running beside it would read from that stub rather than from
//! the shell it means to.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use soloist_core::ShellEnvProbe;
use soloist_sys::CommandShellEnvProbe;

/// How long the background job holds the capture's output pipe open after the shell that started it
/// has gone. Far past the capture's own limit, so a capture that waits for the pipe rather than for
/// its limit is unmistakable, and short enough that one reports itself in seconds.
const HOLDER_LIFE: &str = "10";

/// The capture's limit here — short, because what is under test is that it is what bounds the call.
const TIME_LIMIT: Duration = Duration::from_secs(1);

/// The longest the capture may take. Well past its own limit, so ordinary slowness is never
/// reported as a failure, and well below the background job's life, so waiting that out is.
const ENDS_WITHIN: Duration = Duration::from_secs(5);

#[test]
fn a_capture_whose_output_pipe_is_held_open_still_ends_within_its_time_limit() {
    let dir = tempfile::tempdir().expect("temp dir");
    // A stand-in for the login shell that behaves like one whose startup files leave a background
    // job running: the job inherits the pipe the capture reads, so that pipe stays open long after
    // the shell has exited and there is nothing more to come out of it.
    let shell = dir.path().join("stub-login-shell");
    fs::write(
        &shell,
        format!("#!/bin/sh\nsleep {HOLDER_LIFE} &\nprintf 'PATH=%s\\0' \"$PATH\"\n"),
    )
    .expect("write the stub shell");
    fs::set_permissions(&shell, fs::Permissions::from_mode(0o755)).expect("chmod +x");
    std::env::set_var("SHELL", &shell);

    let started = Instant::now();
    let _ = CommandShellEnvProbe::with_timeout(TIME_LIMIT).capture();
    let took = started.elapsed();

    assert!(
        took < ENDS_WITHIN,
        "the capture has to end at its own limit rather than wait out whatever is still holding \
         its output pipe open: took {took:?}",
    );
}
