//! In-memory agents-context fakes: a [`FakeAgentToolRepo`] holding a fixed tool set and a
//! [`FakeVersionProbe`] reporting a fixed set of commands as installed — so the registry and
//! auto-detection logic are exercised headless, with no SQLite and no real subprocess.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::agents::{AgentTool, AgentToolRepo, Detection, VersionProbe};
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
/// auto-detection tests — no real `--version` subprocess is run. It counts its probes so a
/// test can assert the detection cache reused a sweep instead of re-probing.
pub struct FakeVersionProbe {
    installed: HashSet<String>,
    probes: AtomicUsize,
}

impl FakeVersionProbe {
    /// A probe that reports each command in `installed` as present and all others absent.
    pub fn new(installed: &[&str]) -> Self {
        Self {
            installed: installed.iter().map(|s| s.to_string()).collect(),
            probes: AtomicUsize::new(0),
        }
    }

    /// How many times [`VersionProbe::probe`] has been called across the probe's life.
    pub fn probes(&self) -> usize {
        self.probes.load(Ordering::SeqCst)
    }
}

impl VersionProbe for FakeVersionProbe {
    fn probe(&self, command: &str) -> Detection {
        self.probes.fetch_add(1, Ordering::SeqCst);
        if self.installed.contains(command) {
            Detection::Installed
        } else {
            Detection::Missing
        }
    }
}
