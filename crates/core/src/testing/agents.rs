//! In-memory agents-context fakes: a [`FakeAgentToolRepo`] holding a fixed tool set, a
//! [`FakeVersionProbe`] reporting a fixed set of commands as installed, and a
//! [`FakeAgentOneShot`] answering every headless run with a canned reply — so the registry,
//! auto-detection, and everything composed on a drafting run are exercised headless, with no
//! SQLite and no real subprocess.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::agents::{
    AgentOneShot, AgentTool, AgentToolRepo, Detection, OneShotError, OneShotInvocation,
    VersionProbe,
};
use crate::ports::StoreError;
use crate::sync::lock;

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

/// An [`AgentOneShot`] answering every run the same way, with no subprocess — so what a caller
/// composed for an agent to read, and what it does with the answer, are both testable headless.
///
/// It keeps every run it was asked to make. That is what lets a test read the subject a caller
/// actually built, and lets a test that expects **no** run at all say so by finding none.
pub struct FakeAgentOneShot {
    reply: Result<String, OneShotError>,
    runs: Mutex<Vec<(OneShotInvocation, BTreeMap<String, String>)>>,
}

impl FakeAgentOneShot {
    /// A runner that answers every run with `reply`.
    pub fn answering(reply: &str) -> Self {
        Self::new(Ok(reply.to_string()))
    }

    /// A runner that refuses every run with `err` — a tool that is not installed, one that ran out
    /// of time, one that failed.
    pub fn refusing(err: OneShotError) -> Self {
        Self::new(Err(err))
    }

    fn new(reply: Result<String, OneShotError>) -> Self {
        Self {
            reply,
            runs: Mutex::new(Vec::new()),
        }
    }

    /// Every run it was asked to make, in order — empty when a caller was refused before it ever got
    /// here, which is what an opt-in that nobody opted into looks like from outside.
    pub fn runs(&self) -> Vec<OneShotInvocation> {
        lock(&self.runs)
            .iter()
            .map(|(invocation, _)| invocation.clone())
            .collect()
    }

    /// The environment each run was to be made in, in order — what a caller resolved for a tool to
    /// be found and to read the project the way the user's own terminal would.
    pub fn environments(&self) -> Vec<BTreeMap<String, String>> {
        lock(&self.runs)
            .iter()
            .map(|(_, env)| env.clone())
            .collect()
    }

    /// What the tool was given to read on each run: the standard input where the invocation writes
    /// it there, otherwise the whole command line it was appended to.
    pub fn subjects(&self) -> Vec<String> {
        self.runs()
            .into_iter()
            .map(|run| run.input.unwrap_or(run.command_line))
            .collect()
    }
}

impl AgentOneShot for FakeAgentOneShot {
    fn run(
        &self,
        invocation: &OneShotInvocation,
        _working_dir: &Path,
        env: &BTreeMap<String, String>,
    ) -> Result<String, OneShotError> {
        lock(&self.runs).push((invocation.clone(), env.clone()));
        self.reply.clone()
    }
}
