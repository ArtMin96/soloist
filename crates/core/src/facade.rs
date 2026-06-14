//! The public command and query API that adapters call.
//!
//! [`Facade`] is the one surface every adapter (Tauri, MCP, HTTP/CLI) talks to, so a
//! behaviour like "stop this process" is implemented exactly once. It owns the
//! ports and the event bus, routes commands to the owning context, and exposes cheap
//! queries. The walking skeleton implements one demo command thread end to end.

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::events::{DomainEvent, EventBus};
use crate::ids::ProcessId;
use crate::ports::{Clock, ProcessSpawner, SpawnSpec, Store};
use crate::process::{ProcStatus, ProcessKind, ProcessView};
use crate::supervisor::{spawn_supervised, Registry};

/// Per-subscriber event buffer. Bounded so a stalled adapter re-syncs from a
/// snapshot (see [`crate::events`]) rather than growing memory without limit.
const EVENT_BUFFER: usize = 1024;

/// The walking-skeleton demo command: a process that simply sleeps so its lifecycle
/// (start → run → stop) can be driven end to end.
const DEMO_PROGRAM: &str = "sleep";
const DEMO_ARGS: &[&str] = &["60"];

/// The integration façade (context C8): holds the ports and the process registry and
/// exposes the command/query API. Cheap to clone-share behind an `Arc`.
pub struct Facade {
    spawner: Arc<dyn ProcessSpawner>,
    clock: Arc<dyn Clock>,
    store: Arc<dyn Store>,
    bus: EventBus,
    registry: Registry,
}

impl Facade {
    /// Builds a façade over the given port adapters (real ones in the app, fakes in
    /// tests).
    pub fn new(
        spawner: Arc<dyn ProcessSpawner>,
        clock: Arc<dyn Clock>,
        store: Arc<dyn Store>,
    ) -> Self {
        Self {
            spawner,
            clock,
            store,
            bus: EventBus::new(EVENT_BUFFER),
            registry: Registry::default(),
        }
    }

    /// Subscribes to the domain event stream. Pair with [`Facade::snapshot`]:
    /// read the snapshot first, then apply events (snapshot-then-deltas).
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.bus.subscribe()
    }

    /// The current read model: every known process. Cheap; never blocks writers.
    pub fn snapshot(&self) -> Vec<ProcessView> {
        self.registry.snapshot()
    }

    /// The durable store port (the walking-skeleton seed; adapters use it directly
    /// to prove the storage thread).
    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    /// Spawns the demo process (`sleep 60`) end to end: registers it as `Starting`,
    /// emits [`DomainEvent::ProcessSpawned`], and starts its supervised actor.
    /// Returns its id. Must be called from within a `tokio` runtime.
    pub fn spawn_demo_process(&self) -> ProcessId {
        let id = ProcessId::next();
        let label = format!("demo {id}");
        let view = ProcessView {
            id,
            kind: ProcessKind::Command,
            label: label.clone(),
            status: ProcStatus::Starting,
        };
        let cancel = CancellationToken::new();
        self.registry.insert(view, cancel.clone());
        self.bus.publish(DomainEvent::ProcessSpawned {
            id,
            kind: ProcessKind::Command,
            label,
            status: ProcStatus::Starting,
        });

        spawn_supervised(
            id,
            SpawnSpec {
                program: DEMO_PROGRAM.into(),
                args: DEMO_ARGS.iter().map(|a| (*a).to_string()).collect(),
            },
            self.spawner.clone(),
            self.clock.clone(),
            self.bus.clone(),
            self.registry.clone(),
            cancel,
        );
        id
    }

    /// Requests a graceful stop of the process. Returns whether it was found.
    pub fn stop(&self, id: ProcessId) -> bool {
        self.registry.cancel(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{StoreError, TokioClock};
    use crate::testing::FakeSpawner;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tokio::sync::broadcast::error::RecvError;

    #[derive(Default)]
    struct FakeStore {
        map: Mutex<HashMap<String, String>>,
    }

    impl Store for FakeStore {
        fn meta_get(&self, key: &str) -> Result<Option<String>, StoreError> {
            Ok(crate::sync::lock(&self.map).get(key).cloned())
        }
        fn meta_set(&self, key: &str, value: &str) -> Result<(), StoreError> {
            crate::sync::lock(&self.map).insert(key.into(), value.into());
            Ok(())
        }
    }

    #[tokio::test]
    async fn spawn_demo_registers_and_announces_a_process() {
        let facade = Facade::new(
            Arc::new(FakeSpawner::exits_on_kill()),
            Arc::new(TokioClock),
            Arc::new(FakeStore::default()),
        );
        let mut rx = facade.subscribe();

        let id = facade.spawn_demo_process();

        // It appears in the snapshot immediately, as Starting.
        let snap = facade.snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].id, id);
        assert_eq!(snap[0].status, ProcStatus::Starting);

        // The spawn is announced on the bus.
        match rx.recv().await {
            Ok(DomainEvent::ProcessSpawned {
                id: got, status, ..
            }) => {
                assert_eq!(got, id);
                assert_eq!(status, ProcStatus::Starting);
            }
            other => panic!("expected ProcessSpawned, got {other:?}"),
        }

        // stop() finds the process; stopping an unknown id does not.
        assert!(facade.stop(id));
        assert!(!facade.stop(ProcessId::next()));

        // Drain a couple of events to confirm the stream stays usable.
        loop {
            match rx.recv().await {
                Ok(DomainEvent::ProcessStatusChanged {
                    to: ProcStatus::Stopping,
                    ..
                }) => break,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("bus closed"),
            }
        }
    }
}
