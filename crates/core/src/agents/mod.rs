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

pub use detect::{DetectedTool, Detection, NoopVersionProbe, VersionProbe};
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

    /// Each configured tool paired with what probing `<command> --version` revealed. A tool
    /// outside the auto-detect set is never probed and reports [`Detection::Unknown`]; a probe
    /// that cannot reach an answer reports the same rather than failing the sweep.
    ///
    /// The probes run **off** the async runtime and **concurrently** — one blocking task each —
    /// so the sweep costs about one probe rather than their sum. That matters because a probe
    /// pays the user's login-shell startup, which dominates its duration. Results are returned
    /// in the registry's stable order regardless of which probe finished first. The sweep is
    /// cached for [`DETECT_CACHE_TTL`], so repeated picker opens reuse one round of probes; an
    /// explicit refresh goes through [`Self::redetect_installed`]. Must run within a `tokio`
    /// runtime.
    pub async fn detect_installed(&self) -> Result<Vec<DetectedTool>, StoreError> {
        self.detect_cache
            .get_or_try_init(|| async {
                let tools = self.tools.list()?;
                Ok(self.probe_all(tools).await)
            })
            .await
    }

    /// A detection sweep that ignores the cache: the cached result is discarded and the CLIs
    /// are probed again. What the UI's explicit "detect" action routes to — a deliberate
    /// "check again" must actually check, or a stale wrong answer (every tool reported absent
    /// because the probe was failing) is unfixable until the TTL lapses.
    pub async fn redetect_installed(&self) -> Result<Vec<DetectedTool>, StoreError> {
        self.detect_cache.invalidate().await;
        self.detect_installed().await
    }

    /// Probes every tool concurrently, one blocking task each, and restores the input order.
    async fn probe_all(&self, tools: Vec<AgentTool>) -> Vec<DetectedTool> {
        let mut probes = tokio::task::JoinSet::new();
        for (position, tool) in tools.into_iter().enumerate() {
            let probe = self.version_probe.clone();
            probes.spawn_blocking(move || {
                let detection = if tool.kind.auto_detectable() {
                    probe.probe(&tool.command)
                } else {
                    Detection::Unknown
                };
                (position, DetectedTool { tool, detection })
            });
        }

        let mut detected = Vec::with_capacity(probes.len());
        while let Some(joined) = probes.join_next().await {
            match joined {
                Ok(result) => detected.push(result),
                // A blocking task cannot be cancelled, so a join error is always a panic;
                // re-raise it so the supervised loop's panic-isolation boundary catches it.
                Err(join_err) => std::panic::resume_unwind(join_err.into_panic()),
            }
        }
        detected.sort_by_key(|(position, _)| *position);
        detected.into_iter().map(|(_, tool)| tool).collect()
    }
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
