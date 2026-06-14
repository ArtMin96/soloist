//! Integration tests against real OS processes: prove the spawner contains a child
//! in its own process group and reaps it on stop, and prove the whole walking
//! skeleton (facade → actor → real spawner → real clock → event bus) runs end to end.

use std::sync::Arc;

use nix::errno::Errno;
use nix::sys::signal::killpg;
use nix::unistd::Pid;
use soloist_core::{
    DomainEvent, Facade, ProcStatus, ProcessSpawner, SpawnSpec, Store, StoreError, TokioClock,
};
use soloist_pty::TokioProcessSpawner;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::{timeout, Duration};

/// A do-nothing [`Store`] so the facade can be built without the SQLite adapter.
struct NullStore;

impl Store for NullStore {
    fn meta_get(&self, _key: &str) -> Result<Option<String>, StoreError> {
        Ok(None)
    }
    fn meta_set(&self, _key: &str, _value: &str) -> Result<(), StoreError> {
        Ok(())
    }
}

const SIGTERM: i32 = nix::libc::SIGTERM;

#[tokio::test]
async fn spawns_into_a_group_and_reaps_it_on_terminate() {
    let spawner = TokioProcessSpawner;
    let mut spawned = spawner
        .spawn(&SpawnSpec {
            program: "sleep".into(),
            args: vec!["30".into()],
        })
        .await
        .expect("spawn sleep");

    let pid = spawned.pid.expect("a real pid");
    let pgid = Pid::from_raw(pid as i32);

    // A graceful SIGTERM to the whole group terminates `sleep`.
    spawned.control.terminate().await.expect("terminate group");
    let status = timeout(Duration::from_secs(5), spawned.exit)
        .await
        .expect("child exits promptly on SIGTERM");
    assert_eq!(status.signal, Some(SIGTERM), "killed by SIGTERM");

    // After the child is reaped, its process group no longer exists.
    assert_eq!(
        killpg(pgid, None).err(),
        Some(Errno::ESRCH),
        "process group must be gone after reaping"
    );
}

#[tokio::test]
async fn forceful_kill_reaps_a_signal_resistant_child() {
    let spawner = TokioProcessSpawner;
    // A child that ignores SIGTERM still dies to SIGKILL on the group.
    let mut spawned = spawner
        .spawn(&SpawnSpec {
            program: "bash".into(),
            args: vec!["-c".into(), "trap '' TERM; sleep 30".into()],
        })
        .await
        .expect("spawn bash");
    let pgid = Pid::from_raw(spawned.pid.expect("pid") as i32);

    spawned.control.kill().await.expect("kill group");
    let _ = timeout(Duration::from_secs(5), spawned.exit)
        .await
        .expect("child exits on SIGKILL");
    assert_eq!(killpg(pgid, None).err(), Some(Errno::ESRCH));
}

#[tokio::test]
async fn facade_runs_the_full_thread_with_real_spawner_and_clock() {
    let facade = Facade::new(
        Arc::new(TokioProcessSpawner),
        Arc::new(TokioClock),
        Arc::new(NullStore),
    );
    let mut events = facade.subscribe();

    let id = facade.spawn_demo_process();

    // Starting was announced via ProcessSpawned; the actor reaches Running.
    wait_for_status(&mut events, ProcStatus::Running).await;

    // Stop sends SIGTERM; the real `sleep` exits within the grace window.
    assert!(facade.stop(id), "stop finds the process");
    wait_for_status(&mut events, ProcStatus::Stopping).await;
    wait_for_status(&mut events, ProcStatus::Stopped).await;

    let status = facade
        .snapshot()
        .into_iter()
        .find(|view| view.id == id)
        .map(|view| view.status);
    assert_eq!(status, Some(ProcStatus::Stopped));
}

async fn wait_for_status(events: &mut Receiver<DomainEvent>, target: ProcStatus) {
    let deadline = Duration::from_secs(10);
    let found = timeout(deadline, async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::ProcessStatusChanged { to, .. }) if to == target => return true,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return false,
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(found, "expected to observe status {target:?}");
}
