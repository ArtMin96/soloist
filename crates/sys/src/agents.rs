//! Auto-detecting installed agent CLIs: the OS read behind the core's [`VersionProbe`].
//!
//! Runs `<command> --version` and reports whether it exits successfully — Soloist's signal
//! that the provider's CLI is on this machine. Best-effort and bounded: a missing binary, a
//! non-zero exit, or a hang past the timeout all report not-installed, and a hung probe is
//! killed and reaped so it never leaks. The probe blocks (it spawns and waits on a child), so
//! the core runs it off the async runtime.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use soloist_core::VersionProbe;

/// How long to wait for `--version` before treating the CLI as unavailable. A real
/// `--version` returns near-instantly; the ceiling only guards against a pathological hang.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

/// How often to poll the child while waiting, between spawn and the timeout.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Detects installed agent CLIs by running their `--version`. Stateless; the timeout bounds
/// each probe.
pub struct CommandVersionProbe {
    timeout: Duration,
}

impl CommandVersionProbe {
    /// A probe with the default `--version` timeout.
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// A probe with an explicit timeout (tests use a short one to exercise the hang path
    /// without waiting the full default).
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for CommandVersionProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionProbe for CommandVersionProbe {
    fn is_installed(&self, command: &str) -> bool {
        runs_version_ok(command, self.timeout)
    }
}

/// Whether `command --version` spawns and exits 0 within `timeout`. A spawn failure (missing
/// binary) or a non-zero exit is `false`; a child still running at the deadline is killed,
/// reaped, and reported `false`.
fn runs_version_ok(command: &str, timeout: Duration) -> bool {
    let mut child = match Command::new(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        // The binary is not on PATH (or not executable) — simply not installed.
        Err(_) => return false,
    };

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if Instant::now() >= deadline {
                    // Past the ceiling: kill and reap so the probe never leaks a process.
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(_) => return false,
        }
    }
}
