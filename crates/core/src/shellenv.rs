//! Capturing the user's login-shell environment so managed processes see what a real
//! terminal would.
//!
//! A command launched as `$SHELL -lc <command>` runs a *login* shell, which sources the
//! login profile but not the interactive rc files (`.bashrc`, `.zshrc`) where version
//! managers like nvm, rbenv, and pyenv usually put themselves on `PATH`. To make those
//! tools visible, the environment of an *interactive login* shell (`$SHELL -ilc env`) is
//! captured once and layered onto each spawn.
//!
//! The capture itself — running a shell — is an OS side-effect behind the
//! [`ShellEnvProbe`] port. The policy here is pure and therefore testable on a mock
//! [`Clock`] with no real shell: a time-bounded cache so the shell runs at most once per
//! [`CACHE_TTL`] rather than on every spawn, the precedence by which a process's own
//! `env`, the captured shell environment, and the app environment are layered, and a
//! fallback that prepends the common user bin directories to `PATH` when the capture
//! fails so a process still launches.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::ports::Clock;
use crate::supervision::run_blocking;

/// How long a captured environment is reused before the shell is run again. Long enough
/// that a burst of process starts triggers a single capture, short enough that editing a
/// shell rc file is reflected within minutes.
const CACHE_TTL: Duration = Duration::from_secs(600);

/// Directories prepended to `PATH` only when the shell capture fails, so common
/// user-installed binaries are still found without it. A successful capture carries the
/// shell's own, richer `PATH` and never consults these. A leading `~` is expanded against
/// the app environment's `HOME`; an unresolvable `~` entry is dropped.
const FALLBACK_PATH_DIRS: [&str; 2] = ["~/.local/bin", "/usr/local/bin"];

/// The environment variable whose value is a colon-separated search path.
const PATH_VAR: &str = "PATH";

/// Errors a shell-environment capture surfaces. A failure is never fatal — the resolver
/// falls back to the app environment — so the variant only carries a reason to log.
#[derive(Debug, thiserror::Error)]
pub enum ShellEnvError {
    /// The shell could not be run, or its output could not be read or parsed into any
    /// variables.
    #[error("shell environment capture failed: {0}")]
    Capture(String),
}

/// Captures the environment of the user's interactive login shell — the OS read behind
/// the resolver. The call blocks (it spawns a shell and waits for it), so the core runs
/// it off the async runtime; a real adapter bounds it with a timeout and reaps a hung
/// shell. [`NoopShellEnvProbe`] captures nothing, for compositions without the subsystem.
pub trait ShellEnvProbe: Send + Sync {
    /// The variables an interactive login shell exports, or an error if the shell could
    /// not be run or no variables could be read from it.
    fn capture(&self) -> Result<BTreeMap<String, String>, ShellEnvError>;
}

/// A [`ShellEnvProbe`] that captures nothing — the default when shell-environment capture
/// is not wired. With it the resolver contributes no captured variables and never takes
/// the fallback path, so a process's environment is exactly its own `env` overrides over
/// the inherited app environment (the behavior before capture existed).
#[derive(Clone, Copy, Default)]
pub struct NoopShellEnvProbe;

impl ShellEnvProbe for NoopShellEnvProbe {
    fn capture(&self) -> Result<BTreeMap<String, String>, ShellEnvError> {
        Ok(BTreeMap::new())
    }
}

/// Resolves the environment overrides a managed process launches with: the captured
/// login-shell environment with the process's own `env` applied on top. Holds a
/// [`CACHE_TTL`]-bounded cache of the capture, shared behind an `Arc` by the supervisor so
/// every spawn consults the one cache and the shell runs at most once per interval.
pub(crate) struct ShellEnv {
    probe: Arc<dyn ShellEnvProbe>,
    clock: Arc<dyn Clock>,
    /// The app's own environment, captured once at the composition root. It is the
    /// spawner's inherited base, so the resolver only needs it for the fallback `PATH`
    /// (and `HOME`, to expand a `~` in [`FALLBACK_PATH_DIRS`]).
    app_env: BTreeMap<String, String>,
    /// The cached capture, reused until older than [`CACHE_TTL`]. Behind a mutex held
    /// across a capture so a burst of concurrent spawns runs one shell, not many.
    cache: Mutex<Option<Cached>>,
}

/// One cached capture and when it was taken (per the resolver's [`Clock`]).
struct Cached {
    env: BTreeMap<String, String>,
    at: Instant,
}

impl ShellEnv {
    /// Builds a resolver over the capture `probe`, a `clock` (for the cache TTL), and the
    /// `app_env` captured at the composition root.
    pub(crate) fn new(
        probe: Arc<dyn ShellEnvProbe>,
        clock: Arc<dyn Clock>,
        app_env: BTreeMap<String, String>,
    ) -> Self {
        Self {
            probe,
            clock,
            app_env,
            cache: Mutex::new(None),
        }
    }

    /// The full set of environment overrides for a spawn: the captured login-shell
    /// environment with `process_env` applied on top. The spawner inherits the app
    /// environment and layers these, giving the documented precedence — per-process `env`
    /// wins over the captured shell environment, which wins over the app environment.
    pub(crate) async fn resolve(
        &self,
        process_env: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let mut env = self.captured_layer().await;
        for (key, value) in process_env {
            env.insert(key.clone(), value.clone());
        }
        env
    }

    /// The captured layer, served from the cache when fresh and otherwise recaptured. On a
    /// capture failure it is the [`fallback`](Self::fallback) `PATH` override.
    async fn captured_layer(&self) -> BTreeMap<String, String> {
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.as_ref() {
            if self.clock.now().saturating_duration_since(cached.at) < CACHE_TTL {
                return cached.env.clone();
            }
        }
        // Stale or never captured: run the shell once while holding the lock, so a burst
        // of concurrent spawns waits for this one capture instead of each running its own.
        // A successful capture is cached for the full TTL; a failure is not cached, so the
        // next spawn retries rather than locking in a transient failure for ten minutes.
        let probe = self.probe.clone();
        match run_blocking(move || probe.capture()).await {
            Ok(captured) => {
                let entry = Cached {
                    env: captured.clone(),
                    at: self.clock.now(),
                };
                *cache = Some(entry);
                captured
            }
            Err(_) => self.fallback(),
        }
    }

    /// The fallback captured layer when the shell capture fails: a single `PATH` override
    /// that prepends the common user bin directories to the app environment's `PATH`. The
    /// app environment itself is the spawner's inherited base, so only `PATH` needs
    /// contributing here.
    fn fallback(&self) -> BTreeMap<String, String> {
        let home = self.app_env.get("HOME").map(String::as_str);
        let mut dirs = expand_dirs(&FALLBACK_PATH_DIRS, home);
        if let Some(existing) = self.app_env.get(PATH_VAR) {
            if !existing.is_empty() {
                dirs.push(existing.clone());
            }
        }
        let mut env = BTreeMap::new();
        env.insert(PATH_VAR.to_string(), dirs.join(":"));
        env
    }
}

/// Expands directory templates, replacing a leading `~/` with `home`. An entry that needs
/// `home` but has none is dropped rather than left as a literal `~`.
fn expand_dirs(dirs: &[&str], home: Option<&str>) -> Vec<String> {
    dirs.iter()
        .filter_map(|dir| match dir.strip_prefix("~/") {
            Some(rest) => home.map(|home| format!("{home}/{rest}")),
            None => Some((*dir).to_string()),
        })
        .collect()
}

#[cfg(test)]
#[path = "shellenv_tests.rs"]
mod tests;
