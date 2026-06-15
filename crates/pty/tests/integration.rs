//! Integration tests against real OS processes: prove the spawner runs commands in
//! their working dir with their env, contains a child in its own process group and
//! reaps it (and its grandchildren) on stop, and that the whole supervisor thread
//! (façade → actor → real spawner → real clock → event bus) runs end to end.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nix::errno::Errno;
use nix::sys::signal::killpg;
use nix::unistd::Pid;
use soloist_core::{
    DomainEvent, Facade, Hash, ProcStatus, ProcessSpawner, ProjectId, ProjectRecord, ProjectRepo,
    SpawnSpec, StoreError, TokioClock, TrustRepo,
};
use soloist_pty::TokioProcessSpawner;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::{timeout, Duration};

const SIGTERM: i32 = nix::libc::SIGTERM;

fn spec(command: &str, working_dir: PathBuf) -> SpawnSpec {
    SpawnSpec {
        command: command.into(),
        working_dir,
        env: BTreeMap::new(),
    }
}

#[tokio::test]
async fn runs_a_command_in_its_working_dir_with_its_env() {
    // A marker file in the working dir is visible by a *relative* path only if the
    // child's cwd is the one we set; the env override is checked in the same command.
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("marker"), b"x").expect("write marker");

    let spawner = TokioProcessSpawner;
    let mut launch = spec(
        "test -f marker && test \"$SOLOIST_TEST\" = ok",
        dir.path().to_path_buf(),
    );
    launch.env.insert("SOLOIST_TEST".into(), "ok".into());

    let spawned = spawner.spawn(&launch).await.expect("spawn");
    let status = timeout(Duration::from_secs(5), spawned.exit)
        .await
        .expect("command exits");
    assert_eq!(
        status.code,
        Some(0),
        "command must see its working dir and env"
    );
}

#[tokio::test]
async fn spawns_into_a_group_and_reaps_it_on_terminate() {
    let spawner = TokioProcessSpawner;
    let cwd = std::env::current_dir().expect("cwd");
    let mut spawned = spawner
        .spawn(&spec("sleep 30", cwd))
        .await
        .expect("spawn sleep");

    let pid = spawned.pid.expect("a real pid");
    let pgid = Pid::from_raw(pid as i32);

    // A graceful SIGTERM to the whole group terminates the shell and its `sleep` child.
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
    let cwd = std::env::current_dir().expect("cwd");
    // A shell that ignores SIGTERM and keeps a child alive still dies to SIGKILL.
    let mut spawned = spawner
        .spawn(&spec("trap '' TERM; while true; do sleep 1; done", cwd))
        .await
        .expect("spawn shell");
    let pgid = Pid::from_raw(spawned.pid.expect("pid") as i32);

    spawned.control.kill().await.expect("kill group");
    let _ = timeout(Duration::from_secs(5), spawned.exit)
        .await
        .expect("child exits on SIGKILL");
    assert_eq!(killpg(pgid, None).err(), Some(Errno::ESRCH));
}

#[tokio::test]
async fn start_stop_fifty_processes_leaves_no_survivors() {
    let spawner = TokioProcessSpawner;
    let cwd = std::env::current_dir().expect("cwd");
    let mut groups = Vec::new();
    let mut spawned = Vec::new();
    for _ in 0..50 {
        let child = spawner
            .spawn(&spec("sleep 30", cwd.clone()))
            .await
            .expect("spawn");
        groups.push(Pid::from_raw(child.pid.expect("pid") as i32));
        spawned.push(child);
    }

    for mut child in spawned {
        child.control.terminate().await.expect("terminate");
        let _ = timeout(Duration::from_secs(5), child.exit)
            .await
            .expect("reaped");
    }

    for pgid in groups {
        assert_eq!(
            killpg(pgid, None).err(),
            Some(Errno::ESRCH),
            "no process group may survive"
        );
    }
}

#[tokio::test]
async fn facade_runs_the_full_thread_with_real_spawner_and_clock() {
    let facade = Facade::new(
        Arc::new(TokioProcessSpawner),
        Arc::new(TokioClock),
        Arc::new(NoTrust),
        Arc::new(NoProjects),
    );
    let mut events = facade.subscribe();

    let id = facade.spawn_demo_process();
    wait_for_status(&mut events, ProcStatus::Running).await;

    assert!(facade.supervisor().stop(id), "stop finds the process");
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
    let found = timeout(Duration::from_secs(10), async {
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

/// A trust repo that trusts nothing — the demo is an ungated terminal, so the gate is
/// never consulted; this exists only to satisfy the façade's port.
struct NoTrust;

impl TrustRepo for NoTrust {
    fn is_trusted(&self, _project: ProjectId, _variant: &Hash) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn set_trusted(&self, _project: ProjectId, _variant: &Hash) -> Result<(), StoreError> {
        Ok(())
    }
    fn revoke(&self, _project: ProjectId, _variant: &Hash) -> Result<(), StoreError> {
        Ok(())
    }
}

/// A project repo the demo path never touches; present only to build the façade.
struct NoProjects;

impl ProjectRepo for NoProjects {
    fn upsert(
        &self,
        root: &Path,
        name: Option<&str>,
        icon: Option<&Path>,
    ) -> Result<ProjectRecord, StoreError> {
        Ok(ProjectRecord {
            id: ProjectId::from_raw(1),
            root: root.to_path_buf(),
            name: name.map(str::to_owned),
            icon: icon.map(Path::to_path_buf),
        })
    }
    fn list(&self) -> Result<Vec<ProjectRecord>, StoreError> {
        Ok(Vec::new())
    }
    fn get(&self, _id: ProjectId) -> Result<Option<ProjectRecord>, StoreError> {
        Ok(None)
    }
    fn remove(&self, _id: ProjectId) -> Result<(), StoreError> {
        Ok(())
    }
}
