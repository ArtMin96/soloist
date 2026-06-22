//! In-memory agents-context fakes: a [`FakeAgentToolRepo`] holding a fixed tool set and a
//! [`FakeVersionProbe`] reporting a fixed set of commands as installed — so the registry and
//! auto-detection logic are exercised headless, with no SQLite and no real subprocess.

use std::collections::HashSet;

use crate::agents::{AgentTool, AgentToolRepo, VersionProbe};
use crate::ports::StoreError;

/// An [`AgentToolRepo`] returning a fixed list of tools, for headless registry tests.
pub struct FakeAgentToolRepo {
    tools: Vec<AgentTool>,
}

impl FakeAgentToolRepo {
    /// A repository holding `tools`, returned verbatim by [`AgentToolRepo::list`].
    pub fn new(tools: Vec<AgentTool>) -> Self {
        Self { tools }
    }
}

impl AgentToolRepo for FakeAgentToolRepo {
    fn list(&self) -> Result<Vec<AgentTool>, StoreError> {
        Ok(self.tools.clone())
    }
}

/// A [`VersionProbe`] that reports a fixed set of commands as installed, for headless
/// auto-detection tests — no real `--version` subprocess is run.
pub struct FakeVersionProbe {
    installed: HashSet<String>,
}

impl FakeVersionProbe {
    /// A probe that reports each command in `installed` as present and all others absent.
    pub fn new(installed: &[&str]) -> Self {
        Self {
            installed: installed.iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl VersionProbe for FakeVersionProbe {
    fn is_installed(&self, command: &str) -> bool {
        self.installed.contains(command)
    }
}
