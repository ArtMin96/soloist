//! Shared scaffolding for the agents context's unit tests: an [`Agents`] assembled over fakes, so
//! each test names only the collaborator it is about. Lives in one place so the auto-detection and
//! drafting tests build the context the same way rather than each re-rolling five arguments.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::shellenv::{NoopShellEnvProbe, ShellEnv, ShellEnvProbe};
use crate::testing::{FakeAgentToolRepo, MockClock};

use super::{AgentOneShot, AgentTool, Agents, NoopAgentOneShot, NoopVersionProbe, VersionProbe};

/// An [`Agents`] over `tools` and `version_probe`, timed by `clock`, with nothing wired for
/// drafting — what the auto-detection tests need.
pub(crate) fn detecting(
    tools: Vec<AgentTool>,
    version_probe: Arc<dyn VersionProbe>,
    clock: MockClock,
) -> Agents {
    assemble(
        tools,
        version_probe,
        Arc::new(NoopAgentOneShot),
        Arc::new(NoopShellEnvProbe),
        clock,
    )
}

/// An [`Agents`] whose drafting port is `one_shot` and whose runs are made in the environment
/// `shell_env_probe` captures — what the drafting tests need.
pub(crate) fn drafting(
    tools: Vec<AgentTool>,
    one_shot: Arc<dyn AgentOneShot>,
    shell_env_probe: Arc<dyn ShellEnvProbe>,
) -> Agents {
    assemble(
        tools,
        Arc::new(NoopVersionProbe),
        one_shot,
        shell_env_probe,
        MockClock::new(),
    )
}

fn assemble(
    tools: Vec<AgentTool>,
    version_probe: Arc<dyn VersionProbe>,
    one_shot: Arc<dyn AgentOneShot>,
    shell_env_probe: Arc<dyn ShellEnvProbe>,
    clock: MockClock,
) -> Agents {
    let clock = Arc::new(clock);
    Agents::new(
        Arc::new(FakeAgentToolRepo::new(tools)),
        version_probe,
        one_shot,
        Arc::new(ShellEnv::new(
            shell_env_probe,
            clock.clone(),
            BTreeMap::new(),
        )),
        clock,
    )
}
