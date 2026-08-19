use std::path::{Path, PathBuf};

use nix::unistd::getpgid;
use soloist_core::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{CorePorts, DomainEvent, ProcessId, ProjectId, TokioClock};
use soloist_ipc::{socket_path, IpcResponse, PortWaitOutcome, DATA_DIR_ENV};
use tempfile::TempDir;
use tokio::sync::broadcast;

use super::*;

/// The socket path comes from one process-wide environment variable, so the tests that bind it
/// take turns: each holds this while it owns the data directory.
static SOCKET_DIR: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// How long a test waits for a reply that must arrive; generous, since it only bounds a failure.
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);
/// The readiness wait a test asks for when it needs a request to still be running while the
/// server is cancelled — long enough to cancel inside, short enough to end the test promptly.
const IN_FLIGHT_PORT_WAIT_MS: u64 = 500;
/// A port nothing in the test binds, so the wait for it never resolves early.
const NEVER_BOUND_PORT: u16 = 1;

/// A façade over in-memory fakes — an alternate composition root, as the dispatch tests build.
fn facade() -> Arc<Facade> {
    Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ))
}

/// Points the data directory (and so the socket) at `dir`, and hands back where the server binds.
fn redirect_socket_to(dir: &TempDir) -> PathBuf {
    std::env::set_var(DATA_DIR_ENV, dir.path());
    socket_path().expect("a socket path under the temporary data directory")
}

/// Connects once the server has bound, failing the test rather than hanging if it never does.
async fn connect(path: &Path) -> UnixStream {
    let connected = tokio::time::timeout(REPLY_TIMEOUT, async {
        loop {
            match UnixStream::connect(path).await {
                Ok(stream) => return stream,
                Err(_) => tokio::task::yield_now().await,
            }
        }
    })
    .await;
    connected.expect("the server binds its socket")
}

/// Sends one request and waits for its answer, or `None` when none comes. A connection no longer
/// being served can refuse the write outright, close, or reset; to the caller those are one fact.
async fn ask(stream: &mut UnixStream, request: IpcRequest) -> Option<IpcReply> {
    match write_frame(stream, &request).await {
        Ok(()) => read_reply(stream).await,
        Err(_) => None,
    }
}

/// The next frame the server sends, or `None` if the connection ended without one. A clean close
/// and a reset (the server dropped its end with a request still unread) are the same fact to a
/// caller: no answer is coming.
async fn read_reply(stream: &mut UnixStream) -> Option<IpcReply> {
    tokio::time::timeout(REPLY_TIMEOUT, read_frame(stream))
        .await
        .expect("the connection resolves rather than hanging")
        .unwrap_or(None)
}

fn answer(reply: Option<IpcReply>) -> IpcResponse {
    match reply {
        Some(IpcReply::Done(result)) => (*result).expect("a successful answer"),
        other => panic!("expected an answer, got {other:?}"),
    }
}

/// Registers a process the connecting peer authentically runs in — the test process is its own
/// peer, so its real process group is what the server reads off the socket.
fn process_in_our_own_group(facade: &Facade) -> ProcessId {
    let id = facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "shell",
        "sleep 60",
    ));
    let pgid = getpgid(None).expect("our own process group").as_raw();
    facade.supervisor().assign_test_group(id, pgid);
    id
}

/// Waits for the readiness transition a port wait publishes as it starts polling — the signal
/// that the request is genuinely in flight, so a cancellation now lands mid-request.
async fn await_port_wait_started(events: &mut broadcast::Receiver<DomainEvent>, id: ProcessId) {
    tokio::time::timeout(REPLY_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::ReadyStateChanged { id: waiting, .. }) if waiting == id => return,
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => panic!("event bus closed"),
            }
        }
    })
    .await
    .expect("the port wait starts");
}

// The stop: a cancelled server unlinks its socket and takes no further connection, so nothing
// can reach a server the app has stopped.
#[tokio::test]
async fn cancelling_unlinks_the_socket_and_stops_accepting() {
    let _turn = SOCKET_DIR.lock().await;
    let dir = TempDir::new().expect("tempdir");
    let path = redirect_socket_to(&dir);
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(serve(facade(), shutdown.clone()));
    drop(connect(&path).await);

    shutdown.cancel();
    server.await.expect("the server task ends");

    assert!(!path.exists(), "a stopped server leaves no socket behind");
    assert!(
        UnixStream::connect(&path).await.is_err(),
        "a stopped server must take no further connection"
    );
}

// The half the app's exit used to miss entirely: a connection already accepted stops being
// served too, so no request can reach the core after the app has begun shutting down.
#[tokio::test]
async fn cancelling_stops_serving_an_already_accepted_connection() {
    let _turn = SOCKET_DIR.lock().await;
    let dir = TempDir::new().expect("tempdir");
    let path = redirect_socket_to(&dir);
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(serve(facade(), shutdown.clone()));
    let mut client = connect(&path).await;
    assert!(
        matches!(
            answer(ask(&mut client, IpcRequest::Whoami).await),
            IpcResponse::Whoami(_)
        ),
        "the connection is served while the server runs"
    );

    shutdown.cancel();
    server.await.expect("the server task ends");

    assert!(
        ask(&mut client, IpcRequest::Whoami).await.is_none(),
        "a request made after the stop must go unanswered, and the connection close"
    );
}

// The other side of the same rule: stopping means "read no further request", not "abandon the
// one running" — an operation already under way still gets its answer.
#[tokio::test]
async fn a_request_already_running_is_answered_after_the_stop() {
    let _turn = SOCKET_DIR.lock().await;
    let dir = TempDir::new().expect("tempdir");
    let path = redirect_socket_to(&dir);
    let facade = facade();
    let process = process_in_our_own_group(&facade);
    let mut events = facade.subscribe();
    let shutdown = CancellationToken::new();
    let server = tokio::spawn(serve(Arc::clone(&facade), shutdown.clone()));

    let mut client = connect(&path).await;
    assert_eq!(
        answer(ask(&mut client, IpcRequest::BindSessionProcess { process }).await),
        IpcResponse::Acked
    );
    write_frame(
        &mut client,
        &IpcRequest::WaitForBoundPort {
            process,
            port: NEVER_BOUND_PORT,
            timeout_ms: Some(IN_FLIGHT_PORT_WAIT_MS),
        },
    )
    .await
    .expect("write the request");
    await_port_wait_started(&mut events, process).await;

    shutdown.cancel();

    assert_eq!(
        answer(read_reply(&mut client).await),
        IpcResponse::PortWait(PortWaitOutcome::TimedOut),
        "a request already running when the server stopped must still be answered"
    );
    server.await.expect("the server task ends");
}
