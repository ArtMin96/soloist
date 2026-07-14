//! The `soloist` binary, end to end: a real process invocation driving a real ephemeral HTTP
//! server over a temporary runtime file. This is the H4 evidence — `soloist status` prints the
//! live table from a shell, a `restart` reaches the **real** core (observed on the event bus,
//! so it is the same command the UI and MCP run), and the app-down path reports clearly.

use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;
use std::time::Duration;

use soloist_core::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{CorePorts, DomainEvent, Facade, ProcStatus, ProcessId, ProjectId, TokioClock};
use soloist_httpapi::{router, ApiState};
use soloist_ipc::http::{write_runtime, HttpRuntime};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

/// A façade over fakes with one registered (ungated) terminal named `web`, plus the id to watch.
fn fixture() -> (Arc<Facade>, ProcessId) {
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ));
    let id = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "web",
        "sleep 1",
    ));
    (facade, id)
}

/// Runs the real `soloist` binary against the given data dir (where the runtime file lives).
fn soloist(data_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_soloist-cli"))
        .env("SOLOIST_APP_DATA_DIR", data_dir)
        .args(args)
        .output()
        .expect("run the soloist binary")
}

/// Waits (bounded) for `id` to reach `Running` on the event bus.
async fn await_running(events: &mut broadcast::Receiver<DomainEvent>, id: ProcessId) {
    let wait = async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::ProcessStatusChanged {
                    id: changed, to, ..
                }) if changed == id && to == ProcStatus::Running => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), wait)
        .await
        .expect("the process reached Running");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_binary_drives_the_running_app_and_reports_when_it_is_down() {
    let dir = tempfile::tempdir().expect("temp dir");
    // The sole env user in this test binary, so no parallel test races on it; the subprocess
    // is also handed the dir explicitly.
    std::env::set_var("SOLOIST_APP_DATA_DIR", dir.path());

    let (facade, id) = fixture();
    let mut events = facade.subscribe();

    // Serve the same router the app hosts, on an ephemeral loopback port, and record it.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let port = listener.local_addr().expect("local addr").port();
    // The server and the runtime file the real CLI binary reads must agree on the token, so
    // the binary authenticates end to end.
    let token = "shell-test-token";
    let app = router(ApiState::new(Arc::clone(&facade), token));
    tokio::spawn(async move { axum::serve(listener, app).await.expect("serve") });
    write_runtime(HttpRuntime {
        port,
        token: token.to_string(),
    })
    .expect("write the runtime file");

    let data_dir = dir.path().to_path_buf();

    // `status` prints the fixture process from a real shell invocation.
    let probe = data_dir.clone();
    let status = tokio::task::spawn_blocking(move || soloist(&probe, &["status"]))
        .await
        .expect("join");
    assert!(
        status.status.success(),
        "status exits 0: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let table = String::from_utf8_lossy(&status.stdout);
    assert!(table.contains("web"), "status lists the process: {table}");

    // `restart web` reaches the real core — observed on the event bus, proving the wire
    // mutation runs the same command the UI and MCP would (identical behavior).
    let probe = data_dir.clone();
    let restart = tokio::task::spawn_blocking(move || soloist(&probe, &["restart", "web"]))
        .await
        .expect("join");
    assert!(
        restart.status.success(),
        "restart exits 0: {}",
        String::from_utf8_lossy(&restart.stderr)
    );
    await_running(&mut events, id).await;

    // With the runtime file pointing at a closed port, the CLI reports the app is down.
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    write_runtime(HttpRuntime {
        port: closed,
        token: token.to_string(),
    })
    .expect("rewrite the runtime file");
    let probe = data_dir.clone();
    let down = tokio::task::spawn_blocking(move || soloist(&probe, &["status"]))
        .await
        .expect("join");
    assert!(!down.status.success(), "status fails when the app is down");
    assert!(
        String::from_utf8_lossy(&down.stderr).contains("Soloist is not running"),
        "clear app-down message: {}",
        String::from_utf8_lossy(&down.stderr)
    );
}
