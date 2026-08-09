use super::*;
use soloist_ipc::{
    read_frame, write_frame, IpcReply, IpcRequest, IpcResponse, IpcResult, ProgressReport,
};
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

/// Bounds a test about waiting can actually reach — the real ones are half a minute and five
/// minutes, which no test should spend.
const BRIEF: Duration = Duration::from_millis(300);
const NOT_REACHED: Duration = Duration::from_secs(30);

/// Bounds a test's own wait, so a bound that stopped working fails the test instead of hanging it.
async fn within<T>(work: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(NOT_REACHED, work)
        .await
        .expect("the call under test did not come back at all")
}

/// A stand-in app that says `remarks` about the request, `apart` apart, before answering it.
async fn reporting_server(listener: UnixListener, remarks: Vec<String>, apart: Duration) {
    let (mut stream, _addr) = listener.accept().await.expect("accept");
    while let Some(_request) = read_frame::<_, IpcRequest>(&mut stream)
        .await
        .expect("read request")
    {
        for note in &remarks {
            tokio::time::sleep(apart).await;
            let remark = IpcReply::Progress(ProgressReport { note: note.clone() });
            write_frame(&mut stream, &remark).await.expect("write");
        }
        let answer = IpcReply::Done(Box::new(Ok(IpcResponse::Acked)));
        write_frame(&mut stream, &answer).await.expect("write");
    }
}

#[tokio::test]
async fn a_request_that_asked_to_be_told_hears_every_remark_before_its_answer() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let said = vec![
        "Counting objects: 50%".to_string(),
        "Writing objects: 90%".to_string(),
    ];
    let server = tokio::spawn(reporting_server(listener, said.clone(), Duration::ZERO));
    let (reports, mut heard) = mpsc::channel(8);

    let answer = within(
        AppClient::new(None, socket)
            .request_reporting(IpcRequest::GitPush { progress: true }, reports),
    )
    .await
    .expect("the app answers");

    assert_eq!(answer, IpcResponse::Acked);
    let mut told = Vec::new();
    while let Some(note) = heard.recv().await {
        told.push(note);
    }
    assert_eq!(told, said, "what the app said did not reach whoever asked");
    server.abort();
}

#[tokio::test]
async fn a_request_that_keeps_being_told_outlives_the_silence_it_would_otherwise_die_of() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    // Four remarks, each arriving after more than half the patience has already been spent, so the
    // whole exchange takes far longer than a single silent wait would have tolerated.
    let said: Vec<String> = (0..4)
        .map(|at| format!("Writing objects: {at}0%"))
        .collect();
    let apart = BRIEF * 2 / 3;
    let server = tokio::spawn(reporting_server(listener, said, apart));
    let (reports, _heard) = mpsc::channel(8);

    let answer = within(
        AppClient::new(None, socket)
            .waiting(BRIEF, NOT_REACHED)
            .request_reporting(IpcRequest::GitPush { progress: true }, reports),
    )
    .await;

    assert_eq!(
        answer.expect("an operation that keeps saying it is working is waited for"),
        IpcResponse::Acked,
    );
    server.abort();
}

#[tokio::test]
async fn a_request_that_is_told_nothing_still_gives_up_when_the_app_goes_quiet() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    // Accepts the connection and then says nothing at all, ever.
    let server = tokio::spawn(async move {
        let (stream, _addr) = listener.accept().await.expect("accept");
        std::future::pending::<()>().await;
        drop(stream);
    });

    let outcome = within(
        AppClient::new(None, socket)
            .waiting(BRIEF, NOT_REACHED)
            .request(IpcRequest::ListProcesses),
    )
    .await;

    assert!(
        matches!(outcome, Err(ClientError::Timeout)),
        "silence must still run out, or nothing bounds a wedged app",
    );
    server.abort();
}

#[tokio::test]
async fn a_remark_about_a_request_that_never_asked_for_one_is_a_broken_connection() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let server = tokio::spawn(reporting_server(
        listener,
        vec!["unasked for".to_string()],
        Duration::ZERO,
    ));

    let outcome = within(
        AppClient::new(None, socket)
            .waiting(BRIEF, NOT_REACHED)
            .request(IpcRequest::ListProcesses),
    )
    .await;

    assert!(
        matches!(outcome, Err(ClientError::Transport)),
        "a server volunteering remarks nobody asked for is not keeping the protocol",
    );
    server.abort();
}
