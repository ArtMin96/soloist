//! The agents context's own driven port for auto-detection: probing whether a provider's
//! CLI is installed.
//!
//! The core never spawns a subprocess itself — the real adapter (`crates/sys`) runs the
//! probe off the async runtime; the test adapter returns scripted results. Detection is a
//! best-effort signal: any failure (a missing binary, a non-zero exit, a timeout) is
//! reported as not installed, never an error, so it can never take down the core.

use serde::Serialize;

use super::tool::AgentTool;

/// Probes whether an agent CLI is installed by running its `--version`. A driven port so the
/// pure core stays free of process-spawning; the adapter must run the probe **off** the async
/// runtime (a blocking spawn) and must never block the runtime or panic.
pub trait VersionProbe: Send + Sync {
    /// Whether `command --version` runs and exits successfully. Best-effort: a missing
    /// binary, a non-zero exit, or a hang all report `false`.
    fn is_installed(&self, command: &str) -> bool;
}

/// A [`VersionProbe`] that reports nothing installed — the default until the OS adapter is
/// wired (headless tools, tests). Auto-detection then flags every tool absent.
#[derive(Clone, Copy, Default)]
pub struct NoopVersionProbe;

impl VersionProbe for NoopVersionProbe {
    fn is_installed(&self, _command: &str) -> bool {
        false
    }
}

/// A configured agent tool paired with whether its CLI appears installed — the result of
/// auto-detection. The UI shows installed tools as launchable and flags the rest. A tool
/// whose provider is outside the auto-detect set (Copilot, Kimi, and
/// [`AgentKind::Generic`](super::AgentKind::Generic)) is never probed and so always reports
/// `installed: false`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DetectedTool {
    pub tool: AgentTool,
    pub installed: bool,
}
