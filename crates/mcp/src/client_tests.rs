use super::*;
use soloist_ipc::{read_frame, write_frame, IpcRequest, IpcResponse, IpcResult};
use tokio::net::UnixListener;

/// A minimal stand-in for the app: accepts one connection and answers every request with a
/// canned `Acked`, so the test exercises the real socket transport and framing end to end.
async fn echo_server(listener: UnixListener) {
    let (mut stream, _addr) = listener.accept().await.expect("accept");
    while let Some(_request) = read_frame::<_, IpcRequest>(&mut stream)
        .await
        .expect("read request")
    {
        let reply: IpcResult = Ok(IpcResponse::Acked);
        write_frame(&mut stream, &reply).await.expect("write reply");
    }
}

#[tokio::test]
async fn a_request_round_trips_to_a_listening_app() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let server = tokio::spawn(echo_server(listener));

    let client = AppClient::new(None, socket);
    let response = client
        .request(IpcRequest::ListProcesses)
        .await
        .expect("the app answers");
    assert_eq!(response, IpcResponse::Acked);

    drop(client);
    server.abort();
}

#[tokio::test]
async fn a_request_reports_not_running_when_nothing_listens() {
    let dir = tempfile::tempdir().expect("temp dir");
    // A socket path with no server bound to it.
    let client = AppClient::new(None, dir.path().join("absent.sock"));
    let err = client
        .request(IpcRequest::Whoami)
        .await
        .expect_err("there is no server");
    assert!(matches!(err, ClientError::NotRunning));
}

#[tokio::test(start_paused = true)]
async fn a_request_times_out_when_the_app_never_answers() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    // A wedged app: it accepts the connection but never replies. The paused clock auto-
    // advances to the request timeout, so the assertion is deterministic, not wall-clock.
    let server = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.expect("accept");
        std::future::pending::<()>().await;
    });

    let client = AppClient::new(None, socket);
    let err = client
        .request(IpcRequest::Whoami)
        .await
        .expect_err("a silent app must time out");
    assert!(matches!(err, ClientError::Timeout));

    server.abort();
}
