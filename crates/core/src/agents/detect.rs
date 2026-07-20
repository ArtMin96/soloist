//! The agents context's own driven port for auto-detection: probing whether a provider's
//! CLI is installed.
//!
//! The core never spawns a subprocess itself — the real adapter (`crates/sys`) runs the
//! probe off the async runtime; the test adapter returns scripted results. Detection is a
//! best-effort signal that never fails the caller: a probe that could not reach an answer
//! (a timeout, an unrunnable shell) reports [`Detection::Unknown`] rather than an error, so
//! it can never take down the core — and stays distinguishable from a CLI that genuinely is
//! not on this machine.

use serde::Serialize;

use super::tool::AgentTool;

/// What auto-detection learned about one agent CLI.
///
/// [`Detection::Missing`] and [`Detection::Unknown`] are deliberately separate: "the probe ran
/// and this CLI is not here" is a fact about the machine, whereas "the probe could not answer"
/// is a fact about the probe. Collapsing the two hides a broken probe behind a plausible
/// not-installed badge, so the UI can report each honestly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Detection {
    /// The CLI's `--version` ran and exited successfully.
    Installed,
    /// The probe ran to completion and the CLI is not available.
    Missing,
    /// No answer: the probe timed out, could not be run, or this provider is outside the
    /// auto-detect set and so is never probed.
    Unknown,
}

impl Detection {
    /// Whether the CLI was positively detected. [`Detection::Unknown`] is not installed —
    /// callers that need the distinction match on the variant instead.
    pub fn is_installed(self) -> bool {
        matches!(self, Detection::Installed)
    }
}

/// Probes whether an agent CLI is installed by running its `--version`. A driven port so the
/// pure core stays free of process-spawning; the adapter must run the probe **off** the async
/// runtime (a blocking spawn) and must never block the runtime or panic.
pub trait VersionProbe: Send + Sync {
    /// What running `command --version` reveals. Best-effort: the adapter reports
    /// [`Detection::Unknown`] rather than failing when it cannot reach an answer.
    fn probe(&self, command: &str) -> Detection;
}

/// A [`VersionProbe`] that answers nothing — the default until the OS adapter is wired
/// (headless tools, tests). Every tool then reports [`Detection::Unknown`], which is the
/// truth: with no probe wired, nothing has been checked.
#[derive(Clone, Copy, Default)]
pub struct NoopVersionProbe;

impl VersionProbe for NoopVersionProbe {
    fn probe(&self, _command: &str) -> Detection {
        Detection::Unknown
    }
}

/// A configured agent tool paired with what auto-detection found. The UI shows installed tools
/// as launchable, flags absent ones, and reports an unanswered probe as such. A tool whose
/// provider is outside the auto-detect set (Copilot, Kimi, and
/// [`AgentKind::Generic`](super::AgentKind)) is never probed and so reports
/// [`Detection::Unknown`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DetectedTool {
    pub tool: AgentTool,
    pub detection: Detection,
}
