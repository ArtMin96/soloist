//! A [`ShellEnvProbe`] fake: it returns a fixed environment (or a failure) and counts how
//! many times it was asked to capture, so a test can prove the resolver's cache reuses one
//! capture and its fallback runs when a capture fails.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::shellenv::{ShellEnvError, ShellEnvProbe};

/// A [`ShellEnvProbe`] whose result is fixed per constructor. It records each call so a
/// test can assert the resolver captured exactly once within the cache window.
pub struct FakeShellEnvProbe {
    result: Result<BTreeMap<String, String>, ()>,
    calls: AtomicUsize,
}

impl FakeShellEnvProbe {
    /// A probe that always returns `env`.
    pub fn returning(env: BTreeMap<String, String>) -> Self {
        Self {
            result: Ok(env),
            calls: AtomicUsize::new(0),
        }
    }

    /// A probe whose capture always fails, driving the resolver's fallback path.
    pub fn failing() -> Self {
        Self {
            result: Err(()),
            calls: AtomicUsize::new(0),
        }
    }

    /// How many times [`capture`](ShellEnvProbe::capture) has been called.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ShellEnvProbe for FakeShellEnvProbe {
    fn capture(&self) -> Result<BTreeMap<String, String>, ShellEnvError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result
            .clone()
            .map_err(|()| ShellEnvError::Capture("fake capture failure".to_string()))
    }
}
