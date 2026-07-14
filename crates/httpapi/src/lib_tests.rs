use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::timeout;

use soloist_core::testing::{FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{CorePorts, Facade, TokioClock};

use super::{bind_loopback, serve_on, ApiState};

/// A façade over in-memory fakes — enough state for the router to serve.
fn fake_facade() -> Arc<Facade> {
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

/// The live-teardown contract for the HTTP surface: while the server runs its port accepts; when
/// the shutdown signal fires the server drains and returns, and the port then refuses new
/// connections — so a runtime disable of the integration frees the socket with no app restart.
#[tokio::test]
async fn serve_on_frees_the_port_when_the_shutdown_signal_fires() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = oneshot::channel::<()>();
    let server = tokio::spawn(serve_on(
        listener,
        ApiState::new(fake_facade(), "test-token"),
        async move {
            let _ = rx.await;
        },
    ));

    // The port accepts connections while the server is serving.
    TcpStream::connect(addr)
        .await
        .expect("connect while serving");

    // Firing the shutdown signal stops the server; it drains and returns.
    tx.send(()).expect("send shutdown");
    timeout(Duration::from_secs(5), server)
        .await
        .expect("server stopped within the timeout")
        .expect("server task did not panic");

    // The freed port now refuses new connections.
    assert!(
        TcpStream::connect(addr).await.is_err(),
        "a torn-down server must refuse new connections"
    );
}

/// Two consecutive binds must land on different ports: the first holds its port, so the
/// second has to fall back off it. This holds whatever the absolute numbers are — even if
/// the preferred port was already taken or everything fell through to an OS-assigned port.
#[tokio::test]
async fn a_second_bind_falls_back_off_the_first() {
    let first = bind_loopback().await.expect("first bind");
    let first_port = first.local_addr().expect("first addr").port();
    assert!(first.local_addr().expect("first addr").ip().is_loopback());

    let second = bind_loopback().await.expect("second bind");
    let second_port = second.local_addr().expect("second addr").port();

    assert_ne!(
        first_port, second_port,
        "the second bind must not reuse the port the first is holding"
    );
}
