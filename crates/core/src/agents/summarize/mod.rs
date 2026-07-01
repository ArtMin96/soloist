//! Optional, degradable auto-summarization of an idle agent's recent output (part of context C4).
//!
//! When an agent goes idle, a one-line summary of what it was last doing helps a human — or the
//! coordination layer — read a wall of terminals at a glance. Producing that summary needs an LLM,
//! which the core must never hard-depend on, so the whole subsystem is **opt-in and degradable**:
//! it runs only when the user names a summarizer tool in settings, and any absence or failure (no
//! runner wired, an unsupported provider, a missing CLI) simply yields no summary, leaving idle
//! detection heuristic-only.
//!
//! The design keeps provider knowledge in one place and execution in another:
//! * the [`strategy`] composes each provider's headless invocation (the single cited source of how
//!   Claude/Codex/Gemini/OpenCode/Generic summarize; unsupported providers yield none),
//! * the [`SummaryRunner`] port runs a composed invocation off the runtime (the OS adapter),
//! * the [`SummaryReactor`] reacts to the idle event, reads the live opt-in, and publishes a
//!   [`DomainEvent::AgentSummary`](crate::events::DomainEvent::AgentSummary).

mod prompt;
mod reactor;
mod runner;
mod snapshot;
mod strategy;

pub use reactor::SummaryReactor;
pub use runner::{NoopSummaryRunner, SummaryError, SummaryInvocation, SummaryRunner};
pub use snapshot::OutputSnapshot;
