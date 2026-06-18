//! The public command and query API that adapters call (context C8).
//!
//! [`Facade`] is the one surface every adapter (Tauri, MCP, HTTP/CLI) talks to. It
//! owns the event bus and the bounded contexts — process supervision (C2), and the
//! projects/trust/config of C1 — and hands adapters references to them, so a behaviour
//! like "restart" or "is this command trusted" is implemented exactly once. Adapters
//! translate requests in and project the read model out; they hold no business state.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::broadcast;

use crate::config::ConfigEngine;
use crate::events::{DomainEvent, EventBus};
use crate::ids::{ProcessId, ProjectId};
use crate::ports::{
    Clock, NoopLockReleaser, OrphanControl, ProcessSpawner, ProjectRepo, PtySize, RuntimeState,
    SpawnSpec, TrustRepo,
};
use crate::process::{ProcessKind, ProcessView};
use crate::projects::Projects;
use crate::supervisor::{Registration, Supervisor};
use crate::trust::TrustStore;

/// Per-subscriber event buffer. Bounded so a stalled adapter re-syncs from a snapshot
/// (see [`crate::events`]) rather than growing memory without limit.
const EVENT_BUFFER: usize = 1024;

/// The project the walking-skeleton demo process is registered under.
const DEMO_PROJECT: ProjectId = ProjectId::from_raw(1);
/// The walking-skeleton demo command: a long sleep whose lifecycle (start → run →
/// stop) can be driven end to end from the GUI. An ungated terminal, so it needs no
/// trust record; replaced by real config-driven processes when the dashboard lands.
const DEMO_COMMAND: &str = "sleep 60";

/// The integration façade (context C8). Cheap to share as Tauri-managed state.
pub struct Facade {
    bus: EventBus,
    supervisor: Supervisor,
    projects: Projects,
    trust: TrustStore,
    config: ConfigEngine,
}

impl Facade {
    /// Builds a façade over the given port adapters (real ones in the app, fakes in
    /// tests). The trust repository is shared by the supervisor's trust gate, the
    /// trust store, and the config sync engine, so all three agree on what is trusted.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        spawner: Arc<dyn ProcessSpawner>,
        clock: Arc<dyn Clock>,
        trust: Arc<dyn TrustRepo>,
        projects: Arc<dyn ProjectRepo>,
        runtime: Arc<dyn RuntimeState>,
        orphan_control: Arc<dyn OrphanControl>,
    ) -> Self {
        let bus = EventBus::new(EVENT_BUFFER);
        let supervisor = Supervisor::new(
            spawner,
            clock,
            trust.clone(),
            Arc::new(NoopLockReleaser),
            runtime,
            orphan_control,
            bus.clone(),
        );
        Self {
            supervisor,
            projects: Projects::new(projects),
            trust: TrustStore::new(trust.clone()),
            config: ConfigEngine::new(trust, bus.clone()),
            bus,
        }
    }

    /// Subscribes to the domain event stream. Pair with [`Facade::snapshot`]: read the
    /// snapshot first, then apply events (snapshot-then-deltas).
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.bus.subscribe()
    }

    /// The current process read model. Cheap; never blocks writers.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        self.supervisor.snapshot()
    }

    /// The process supervisor (C2) — start/stop/restart and bulk operations.
    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    /// The project registry (C1).
    pub fn projects(&self) -> &Projects {
        &self.projects
    }

    /// The trust gate (C1).
    pub fn trust(&self) -> &TrustStore {
        &self.trust
    }

    /// The `solo.yml` sync engine (C1).
    pub fn config(&self) -> &ConfigEngine {
        &self.config
    }

    /// Registers and starts the demo process end to end, returning its id. Must be
    /// called from within a `tokio` runtime.
    pub fn spawn_demo_process(&self) -> ProcessId {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let id = self.supervisor.register(Registration::launched(
            DEMO_PROJECT,
            ProcessKind::Terminal,
            "demo",
            SpawnSpec {
                command: DEMO_COMMAND.into(),
                working_dir,
                env: BTreeMap::new(),
                size: PtySize::default(),
            },
        ));
        // Starting an ungated terminal cannot fail the trust gate.
        let _ = self.supervisor.start(id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{NoopOrphanControl, NoopRuntimeState, TokioClock};
    use crate::process::ProcStatus;
    use crate::supervisor::SupervisorError;
    use crate::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo};
    use std::path::Path;
    use tokio::sync::broadcast::error::RecvError;

    fn facade(spawner: FakeSpawner) -> (Facade, Arc<FakeTrustRepo>) {
        let trust = Arc::new(FakeTrustRepo::new());
        let facade = Facade::new(
            Arc::new(spawner),
            Arc::new(TokioClock),
            trust.clone(),
            Arc::new(FakeProjectRepo::new()),
            Arc::new(NoopRuntimeState),
            Arc::new(NoopOrphanControl),
        );
        (facade, trust)
    }

    async fn wait_for(rx: &mut broadcast::Receiver<DomainEvent>, target: ProcStatus) {
        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProcessStatusChanged { to, .. }) if to == target => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    }

    #[tokio::test]
    async fn spawn_demo_registers_and_runs_a_process() {
        let (facade, _trust) = facade(FakeSpawner::exits_on_terminate());
        let mut rx = facade.subscribe();

        let id = facade.spawn_demo_process();
        assert_eq!(facade.snapshot().len(), 1);
        wait_for(&mut rx, ProcStatus::Running).await;

        // Stop routes through the same supervisor the snapshot reflects.
        assert!(facade.supervisor().stop(id));
        wait_for(&mut rx, ProcStatus::Stopped).await;
    }

    #[tokio::test]
    async fn the_trust_gate_is_enforced_through_the_facade() {
        let (facade, trust) = facade(FakeSpawner::exits_on_terminate());
        let config =
            crate::config::parse("processes:\n  Web:\n    command: npm run dev\n").expect("parse");
        let spec = config.processes.get("Web").cloned().expect("Web");
        let project = ProjectId::from_raw(1);
        let id = facade.supervisor().register(Registration::command(
            project,
            Path::new("/p"),
            "Web",
            &spec,
        ));

        assert!(matches!(
            facade.supervisor().start(id),
            Err(SupervisorError::Untrusted)
        ));

        trust
            .set_trusted(project, &spec.variant_hash())
            .expect("trust");
        facade.supervisor().start(id).expect("start once trusted");
    }
}
