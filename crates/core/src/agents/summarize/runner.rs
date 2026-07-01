//! The summarizer executor port: runs a composed headless invocation and returns its output.
//!
//! Auto-summarization is optional and degradable. The *what to run* is composed in the core — the
//! per-provider [`strategy`](super::strategy), one cited source of how each provider summarizes.
//! This port is the *how to run it*: a thin OS executor the real adapter (`crates/sys`) provides
//! and the core calls **off** its async runtime. The default [`NoopSummaryRunner`] reports
//! unavailable, so with no adapter wired — or with summarization simply left off — opting in
//! stores the preference but produces no summary, and the core never depends on an LLM.

use thiserror::Error;

/// A composed headless summarizer invocation: the shell command line to run, and any text to feed
/// it on stdin. Built by the per-provider [`strategy`](super::strategy) from the configured tool,
/// model, and prompt — the command line is POSIX-quoted for `$SHELL -lc`, so the adapter runs it
/// verbatim through the login shell (PATH and version managers resolve exactly as for a launched
/// agent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryInvocation {
    /// The command line, quoted to survive `$SHELL -lc <line>` as its intended arguments.
    pub command_line: String,
    /// Text piped to the process's stdin, for a provider whose prompt convention reads it there
    /// (a Generic tool in [`PromptMode::Stdin`](crate::agents::PromptMode)); `None` when the
    /// prompt is carried on the command line.
    pub stdin: Option<String>,
}

/// Why a summary could not be produced. Every variant degrades gracefully — the reactor drops the
/// summary and leaves idle detection heuristic-only; none takes down the core.
#[derive(Debug, Error)]
pub enum SummaryError {
    /// No runner is wired (the [`NoopSummaryRunner`] default), so nothing can summarize.
    #[error("no summarizer is available")]
    Unavailable,
    /// The summarizer CLI did not finish within the runner's time bound and was killed.
    #[error("the summarizer timed out")]
    TimedOut,
    /// The summarizer CLI failed to spawn or exited unsuccessfully.
    #[error("the summarizer failed: {0}")]
    Failed(String),
}

/// Runs a composed [`SummaryInvocation`] headless and returns the captured standard output.
///
/// A driven port (agents C4): the pure core never spawns a process, so the real adapter
/// (`crates/sys`) runs the invocation. The adapter **must** run it off the async runtime, bound
/// its runtime and kill+reap a hung child, and never panic — a summary is best-effort. The
/// returned string is the raw stdout; the reactor trims and bounds it before publishing.
pub trait SummaryRunner: Send + Sync {
    /// Runs `invocation`, returning its captured stdout or a [`SummaryError`].
    fn run(&self, invocation: &SummaryInvocation) -> Result<String, SummaryError>;
}

/// A [`SummaryRunner`] that always reports [`SummaryError::Unavailable`] — the default until the
/// OS adapter is wired. With it, opting into summarization stores the preference but produces no
/// summary, matching "optional and OFF-safe": the core never hard-depends on an LLM.
#[derive(Clone, Copy, Default)]
pub struct NoopSummaryRunner;

impl SummaryRunner for NoopSummaryRunner {
    fn run(&self, _invocation: &SummaryInvocation) -> Result<String, SummaryError> {
        Err(SummaryError::Unavailable)
    }
}
