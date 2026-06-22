//! The agents context's durable port: the agent-tool registry.
//!
//! Defined here, in the agents context, rather than in the shared port layer — the registry
//! is this domain's concern. The real adapter is SQLite (`crates/store`, which seeds the
//! built-in providers on first run); tests use [`NoopAgentToolRepo`] or an in-memory fake.

use crate::ports::StoreError;

use super::tool::AgentTool;

/// Durable registry of configured agent tools (context C4). Keyed by each tool's unique
/// `name`; the built-in providers are seeded by the store on first run.
pub trait AgentToolRepo: Send + Sync {
    /// Every configured tool, in a stable order (the seeded providers first).
    fn list(&self) -> Result<Vec<AgentTool>, StoreError>;
}

/// An [`AgentToolRepo`] holding no tools — the default until the durable store is wired
/// (headless tools, tests that do not exercise the registry). With it the registry is empty.
#[derive(Clone, Copy, Default)]
pub struct NoopAgentToolRepo;

impl AgentToolRepo for NoopAgentToolRepo {
    fn list(&self) -> Result<Vec<AgentTool>, StoreError> {
        Ok(Vec::new())
    }
}
