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

/// A stand-in app that answers each bind request from `binds` in order and everything else with
/// `Acked`, so a test can script a sequence of bind outcomes over one real connection.
async fn scripted_bind_server(listener: UnixListener, binds: Vec<IpcResult>) {
    let (mut stream, _addr) = listener.accept().await.expect("accept");
    let mut binds = binds.into_iter();
    while let Some(request) = read_frame::<_, IpcRequest>(&mut stream)
        .await
        .expect("read request")
    {
        let reply: IpcResult = match request {
            IpcRequest::BindSessionProcess { .. } => binds.next().expect("a scripted bind reply"),
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
        client.note_bind_outcome(
            INJECTED_PROCESS,
            Some(BindFailure::Refused(IpcError::ForeignProcess))
        ),
        None
    );

    drop(client);
    server.abort();
}

#[tokio::test]
async fn a_refusal_that_returns_after_a_successful_bind_is_reported_again() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let server = tokio::spawn(scripted_bind_server(
        listener,
        vec![
            Err(IpcError::UnknownProcess),
            Ok(IpcResponse::Acked),
            Err(IpcError::UnknownProcess),
        ],
    ));

    let client = AppClient::new(Some(INJECTED_PROCESS), socket.clone());
    let mut stream = UnixStream::connect(&socket).await.expect("connect");

    assert!(
        client
            .bind_session(&mut stream, INJECTED_PROCESS)
            .await
            .is_some(),
        "the first refusal is news"
    );
    assert!(
        client
            .bind_session(&mut stream, INJECTED_PROCESS)
            .await
            .is_none(),
        "a bind that succeeds has nothing to report"
    );
    // The session is unbound again, for the reason it was unbound the first time. That is a fresh
    // event, not a repeat of a standing one: every tool that needs an owning process starts
    // refusing, and this line is the only sign of it a user reading the host's log gets.
    assert!(
        client
            .bind_session(&mut stream, INJECTED_PROCESS)
            .await
            .is_some(),
        "a refusal that returns after a successful bind must be reported again"
    );

    drop(client);
    server.abort();
}

#[tokio::test]
async fn a_bind_the_app_never_answered_is_reported_with_the_retry() {
    let dir = tempfile::tempdir().expect("temp dir");
    let client = AppClient::new(Some(INJECTED_PROCESS), dir.path().join("absent.sock"));

    let report = client
        .note_bind_outcome(INJECTED_PROCESS, Some(BindFailure::Unreachable))
        .expect("the first failure is reported");
    assert!(report.contains(&INJECTED_PROCESS.to_string()));
    // Nothing about this session forbids the bind — it simply never reached Soloist — so calling
    // the tool is the remedy, and the report must name it.
    assert!(
        report.contains("bind_session_process"),
        "the report must name the tool that retries a bind that never landed: {report}"
    );
    // A different failure is news and is reported again, so a changed reason is never swallowed.
    assert!(client
        .note_bind_outcome(
            INJECTED_PROCESS,
            Some(BindFailure::Refused(IpcError::UnknownProcess))
        )
        .is_some());
}

#[tokio::test]
async fn a_refusal_no_retry_can_clear_is_not_reported_as_retryable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let client = AppClient::new(Some(INJECTED_PROCESS), dir.path().join("absent.sock"));

    let report = client
        .note_bind_outcome(
            INJECTED_PROCESS,
            Some(BindFailure::Refused(IpcError::ForeignProcess)),
        )
        .expect("a refusal is reported");
    assert!(report.contains(&INJECTED_PROCESS.to_string()));
    assert!(report.contains(&IpcError::ForeignProcess.to_string()));
    // The app refused because this session does not run in that process, which it re-checks the
    // same way on the same connection — so naming the retry here sends the reader round a loop
    // that cannot end differently. The report must name a remedy that can actually work.
    assert!(
        !report.contains("bind_session_process"),
        "the report must not offer a retry that is guaranteed to be refused again: {report}"
    );
    assert!(
        report.contains("Launch it from Soloist"),
        "the report must name what does restore an owning process: {report}"
    );
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
