//! Agents bounded context (C4): the configurable agent CLIs Soloist launches.
//!
//! The context owns the durable **agent-tool registry** (the built-in providers seeded on
//! first run, plus any the user adds) and **`--version` auto-detection** of which providers'
//! CLIs are installed. It owns its own driven ports — [`AgentToolRepo`] (durable) and
//! [`VersionProbe`] (auto-detect) — each with a `Noop` default, so the core runs without the
//! real adapters. Launching agents and the 5-state idle FSM build on these types.

mod detect;
pub mod idle;
mod repo;
mod tool;

pub use detect::{DetectedTool, NoopVersionProbe, VersionProbe};
pub use idle::{AgentActivity, IdleSampler, IdleTracker};
pub use repo::{AgentToolRepo, NoopAgentToolRepo};
pub use tool::{AgentKind, AgentTool, PromptMode};

use std::sync::Arc;

use crate::ports::StoreError;

/// The agents context surface: the agent-tool registry plus auto-detection of which
/// providers' CLIs are installed. Built once by the composition root from its two ports and
/// owned by the [`Facade`](crate::facade::Facade); adapters call into here rather than
/// touching the ports directly.
pub struct Agents {
    tools: Arc<dyn AgentToolRepo>,
    version_probe: Arc<dyn VersionProbe>,
}

impl Agents {
    /// Assembles the context over its durable registry and its auto-detect probe.
    pub fn new(tools: Arc<dyn AgentToolRepo>, version_probe: Arc<dyn VersionProbe>) -> Self {
        Self {
            tools,
            version_probe,
        }
    }

    /// Every configured agent tool (the built-in providers seeded on first run, plus any the
    /// user added), in the registry's stable order.
    pub fn list_tools(&self) -> Result<Vec<AgentTool>, StoreError> {
        self.tools.list()
    }

    /// The configured tool with this unique `name`, or `None` if no tool is registered under
    /// it. The lookup the launch path uses to resolve a picker selection to its definition.
    pub fn tool(&self, name: &str) -> Result<Option<AgentTool>, StoreError> {
        Ok(self
            .tools
            .list()?
            .into_iter()
            .find(|tool| tool.name == name))
    }

    /// Each configured tool paired with whether its CLI appears installed, by probing
    /// `<command> --version`. A [`AgentKind::Generic`] tool is never probed (it is
    /// user-configured) and so reports not-installed. The probes run **off** the async
    /// runtime, so a missing or slow CLI never stalls a runtime worker; a failed probe simply
    /// reports not-installed. Must run within a `tokio` runtime.
    pub async fn detect_installed(&self) -> Result<Vec<DetectedTool>, StoreError> {
        let tools = self.tools.list()?;
        let probe = self.version_probe.clone();
        Ok(crate::supervision::run_blocking(move || {
            tools
                .into_iter()
                .map(|tool| {
                    let installed =
                        tool.kind.auto_detectable() && probe.is_installed(&tool.command);
                    DetectedTool { tool, installed }
                })
                .collect()
        })
        .await)
    }
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
