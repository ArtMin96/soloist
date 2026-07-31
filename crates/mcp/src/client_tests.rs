use super::*;
use soloist_ipc::{read_frame, write_frame, IpcRequest, IpcResponse, IpcResult};
use tokio::net::UnixListener;

/// The process id Soloist injects, which the client binds each fresh connection to.
const INJECTED_PROCESS: ProcessId = ProcessId::from_raw(10);

/// A stand-in app that refuses every bind and answers everything else with `Acked` — the shape a
/// session presents when its peer fails the app's authenticity check.
async fn bind_refusing_server(listener: UnixListener) {
    let (mut stream, _addr) = listener.accept().await.expect("accept");
    while let Some(request) = read_frame::<_, IpcRequest>(&mut stream)
        .await
        .expect("read request")
    {
        let reply: IpcResult = match request {
            IpcRequest::BindSessionProcess { .. } => Err(IpcError::ForeignProcess),
            _ => Ok(IpcResponse::Acked),
        };
        write_frame(&mut stream, &reply).await.expect("write reply");
    }
}

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
async fn a_refused_bind_is_recorded_and_still_serves_the_connection() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let server = tokio::spawn(bind_refusing_server(listener));

    let client = AppClient::new(Some(INJECTED_PROCESS), socket);
    // The refusal must not fail the connection: an unbound session still answers tool calls.
    assert_eq!(
        client
            .request(IpcRequest::ListProcesses)
            .await
            .expect("a refused bind leaves the connection usable"),
        IpcResponse::Acked
    );
    // ...and it was recorded rather than discarded, so re-noting the same refusal is silent: it
    // has already been reported.
    assert_eq!(
        client.note_bind_failure(
            INJECTED_PROCESS,
            BindFailure::Refused(IpcError::ForeignProcess)
        ),
        None
    );

    drop(client);
    server.abort();
}

#[tokio::test]
async fn a_bind_failure_is_reported_with_the_process_the_reason_and_the_way_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let client = AppClient::new(Some(INJECTED_PROCESS), dir.path().join("absent.sock"));

    let report = client
        .note_bind_failure(
            INJECTED_PROCESS,
            BindFailure::Refused(IpcError::ForeignProcess),
        )
        .expect("the first failure is reported");
    assert!(report.contains(&INJECTED_PROCESS.to_string()));
    assert!(report.contains(&IpcError::ForeignProcess.to_string()));
    assert!(
        report.contains("bind_session_process"),
        "the report must name the tool that retries the bind: {report}"
    );
    // A different failure is news and is reported again, so a changed reason is never swallowed.
    assert!(client
        .note_bind_failure(INJECTED_PROCESS, BindFailure::Unreachable)
        .is_some());
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
