//! Agents bounded context (C4): the configurable agent CLIs Soloist launches.
//!
//! The context owns the durable **agent-tool registry** (the built-in providers seeded on
//! first run, plus any the user adds) and **`--version` auto-detection** of which providers'
//! CLIs are installed. It owns its own driven ports — [`AgentToolRepo`] (durable) and
//! [`VersionProbe`] (auto-detect) — each with a `Noop` default, so the core runs without the
//! real adapters. Launching agents and the 5-state idle FSM build on these types.

mod detect;
pub mod idle;
mod lineage;
mod repo;
mod resume;
mod tool;

pub use detect::{DetectedTool, NoopVersionProbe, VersionProbe};
pub use idle::{AgentActivity, IdleSampler, IdleTracker};
pub use lineage::AgentLineage;
pub use repo::{AgentToolRepo, NoopAgentToolRepo};
pub use tool::{AgentKind, AgentTool, PromptMode};

use std::sync::Arc;
use std::time::Duration;

use crate::cache::ReadCache;
use crate::ports::{Clock, StoreError};

/// How long an auto-detection sweep is reused before the CLIs are probed again. CLIs are
/// installed or removed rarely, so a burst of picker opens shares one sweep; a fresh install
/// shows after this window or an app restart. Mirrors the env-capture cadence ([`crate::shellenv`]).
const DETECT_CACHE_TTL: Duration = Duration::from_secs(600);

/// The agents context surface: the agent-tool registry plus auto-detection of which
/// providers' CLIs are installed. Built once by the composition root from its two ports and
/// owned by the [`Facade`](crate::facade::Facade); adapters call into here rather than
/// touching the ports directly.
pub struct Agents {
    tools: Arc<dyn AgentToolRepo>,
    version_probe: Arc<dyn VersionProbe>,
    /// A detection sweep reused for [`DETECT_CACHE_TTL`], so repeated picker opens share one
    /// round of `--version` probes instead of re-running the slow probe each time.
    detect_cache: ReadCache<Vec<DetectedTool>>,
}

impl Agents {
    /// Assembles the context over its durable registry, its auto-detect probe, and a `clock`
    /// driving the detection cache's TTL.
    pub fn new(
        tools: Arc<dyn AgentToolRepo>,
        version_probe: Arc<dyn VersionProbe>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            tools,
            version_probe,
            detect_cache: ReadCache::new(clock, DETECT_CACHE_TTL),
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
    /// reports not-installed. The sweep is cached for [`DETECT_CACHE_TTL`], so repeated picker
    /// opens reuse one round of probes. Must run within a `tokio` runtime.
    pub async fn detect_installed(&self) -> Result<Vec<DetectedTool>, StoreError> {
        self.detect_cache
            .get_or_try_init(|| async {
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
            })
            .await
    }
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
