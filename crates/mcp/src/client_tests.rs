use std::collections::VecDeque;

use super::*;
use soloist_core::{AgentKind, Origin, SessionId};
use soloist_ipc::{
    read_frame, write_frame, IpcError, IpcReply, IpcRequest, IpcResponse, IpcResult, ProgressReport,
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

#[tokio::test]
async fn a_fresh_connection_replays_the_bind_before_the_caller_s_own_request() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.expect("accept");
        let first = read_frame::<_, IpcRequest>(&mut stream)
            .await
            .expect("read first request")
            .expect("a request");
        let reply: IpcResult = Ok(IpcResponse::Acked);
        write_frame(&mut stream, &reply).await.expect("write reply");
        let _second = read_frame::<_, IpcRequest>(&mut stream)
            .await
            .expect("read second request")
            .expect("a request");
        write_frame(&mut stream, &reply).await.expect("write reply");
        first
    });

    let client = AppClient::new(Some(ProcessId::from_raw(9)), socket);
    within(client.request(IpcRequest::ListProcesses))
        .await
        .expect("the app answers");

    let first_request = server.await.expect("server task");
    assert_eq!(
        first_request,
        IpcRequest::BindSessionProcess {
            process: ProcessId::from_raw(9)
        },
        "the first frame on a fresh connection must be the bind",
    );
}

/// A fake app across successive connections: closes each one once it has served as many requests
/// as `closes`' front entry says (popped once per accepted connection; a connection left with no
/// entry is served without limit), and reports every request it served — across every connection,
/// in arrival order — on `seen`, so a test can see what a reconnect replayed before the request
/// that asked for it reached the app.
async fn reconnecting_app(
    listener: UnixListener,
    mut closes: VecDeque<usize>,
    seen: mpsc::Sender<IpcRequest>,
    respond: impl Fn(usize, &IpcRequest) -> IpcResult + Send + 'static,
) {
    let mut connection = 0;
    loop {
        let (mut stream, _addr) = listener.accept().await.expect("accept");
        let close_after = closes.pop_front();
        let mut served = 0;
        while close_after != Some(served) {
            let Some(request) = read_frame::<_, IpcRequest>(&mut stream)
                .await
                .expect("read request")
            else {
                break;
            };
            let reply = respond(connection, &request);
            let _ = seen.send(request).await;
            write_frame(&mut stream, &reply).await.expect("write reply");
            served += 1;
        }
        connection += 1;
    }
}

#[tokio::test]
async fn a_reconnect_replays_the_register_and_project_facts_before_the_next_request() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let (seen, mut heard) = mpsc::channel(32);
    let server = tokio::spawn(reconnecting_app(
        listener,
        VecDeque::from([3]),
        seen,
        |_connection, _request| Ok(IpcResponse::Acked),
    ));

    let client = AppClient::new(Some(ProcessId::from_raw(7)), socket);
    within(client.establishing(IpcRequest::RegisterAgent {
        label: "agent".into(),
    }))
    .await
    .expect("register succeeds");
    within(client.establishing(IpcRequest::SelectProject {
        project: ProjectId::from_raw(2),
    }))
    .await
    .expect("select succeeds");

    // The connection has now carried exactly the three requests the app was told to close after;
    // the next one finds it already gone.
    assert!(within(client.request(IpcRequest::ListProcesses))
        .await
        .is_err());

    // The call after that opens a fresh connection — the app must see it arrive already
    // registered and scoped, before the request that asked for it.
    within(client.request(IpcRequest::ListProcesses))
        .await
        .expect("the reconnect succeeds");

    let mut requests = Vec::new();
    while let Ok(request) = heard.try_recv() {
        requests.push(request);
    }
    assert_eq!(
        requests[3..].to_vec(),
        vec![
            IpcRequest::BindSessionProcess {
                process: ProcessId::from_raw(7)
            },
            IpcRequest::RegisterAgent {
                label: "agent".into()
            },
            IpcRequest::SelectProject {
                project: ProjectId::from_raw(2)
            },
            IpcRequest::ListProcesses,
        ],
        "the second connection must see the earlier facts replayed, in order, before the request \
that made the app close the first one",
    );
    server.abort();
}

#[tokio::test]
async fn a_replay_the_app_refuses_is_not_retried_on_the_connection_after_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let (seen, mut heard) = mpsc::channel(32);
    let server = tokio::spawn(reconnecting_app(
        listener,
        VecDeque::from([3, 4]),
        seen,
        |connection, request| match (connection, request) {
            (1, IpcRequest::SelectProject { .. }) => Err(IpcError::ForeignProject),
            _ => Ok(IpcResponse::Acked),
        },
    ));

    let client = AppClient::new(Some(ProcessId::from_raw(7)), socket);
    within(client.establishing(IpcRequest::RegisterAgent {
        label: "agent".into(),
    }))
    .await
    .expect("register succeeds");
    within(client.establishing(IpcRequest::SelectProject {
        project: ProjectId::from_raw(2),
    }))
    .await
    .expect("select succeeds");

    // First connection is spent (bind, register, select); this finds it gone.
    assert!(within(client.request(IpcRequest::ListProcesses))
        .await
        .is_err());
    // Second connection replays register and the now-refused select, then serves this request —
    // the refusal must not stop the request that triggered the reconnect from succeeding.
    within(client.request(IpcRequest::ListProcesses))
        .await
        .expect("a refused replay must not fail the request that follows it");
    // Second connection is spent too (bind, register, refused select, the request above); this
    // finds it gone.
    assert!(within(client.request(IpcRequest::ListProcesses))
        .await
        .is_err());
    // Third connection: the refused selection must not be retried.
    within(client.request(IpcRequest::ListProcesses))
        .await
        .expect("the third connection still serves the request");

    let mut requests = Vec::new();
    while let Ok(request) = heard.try_recv() {
        requests.push(request);
    }
    assert_eq!(
        requests[7..].to_vec(),
        vec![
            IpcRequest::BindSessionProcess {
                process: ProcessId::from_raw(7)
            },
            IpcRequest::RegisterAgent {
                label: "agent".into()
            },
            IpcRequest::ListProcesses,
        ],
        "a project selection the app refused once must not be replayed onto the next connection",
    );
    server.abort();
}

/// A fake app whose project scope is a fact of the connection, not of the session: `SelectProject`
/// marks the connection it arrived on, cleared on every fresh accept, and a scoped request
/// (modelled here by `GetProjectStatus { project: None }`, the request whose doc already says it
/// reads "the effective scope") answers only when that connection was marked — close enough to the
/// real app's one-connection-one-session rule to prove whether a lost selection gets replayed.
async fn scoped_app(listener: UnixListener, close_first_connection_after: usize) {
    let mut connection = 0;
    loop {
        let (mut stream, _addr) = listener.accept().await.expect("accept");
        let mut project_selected = false;
        let mut served = 0;
        while !(connection == 0 && served == close_first_connection_after) {
            let Some(request) = read_frame::<_, IpcRequest>(&mut stream)
                .await
                .expect("read request")
            else {
                break;
            };
            let reply: IpcResult = match &request {
                IpcRequest::SelectProject { .. } => {
                    project_selected = true;
                    Ok(IpcResponse::Acked)
                }
                IpcRequest::GetProjectStatus { project: None } if project_selected => {
                    Ok(IpcResponse::Acked)
                }
                IpcRequest::GetProjectStatus { project: None } => Err(IpcError::NoProjectScope),
                _ => Ok(IpcResponse::Acked),
            };
            write_frame(&mut stream, &reply).await.expect("write reply");
            served += 1;
        }
        connection += 1;
    }
}

#[tokio::test]
async fn losing_the_connection_after_selecting_a_project_does_not_lose_the_selection() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let server = tokio::spawn(scoped_app(listener, 2));

    let client = AppClient::new(Some(ProcessId::from_raw(4)), socket);
    within(client.establishing(IpcRequest::SelectProject {
        project: ProjectId::from_raw(9),
    }))
    .await
    .expect("select succeeds");

    // Bind and select already used up the two requests the app serves before closing; this finds
    // the connection already gone.
    assert!(
        within(client.request(IpcRequest::GetProjectStatus { project: None }))
            .await
            .is_err()
    );

    // The reconnect must replay the earlier selection onto the fresh connection — without it, this
    // connection starts with no selection and the app refuses with `NoProjectScope`.
    let outcome = within(client.request(IpcRequest::GetProjectStatus { project: None })).await;
    assert!(
        outcome.is_ok(),
        "a project selected before a reconnect must still resolve after it: {outcome:?}",
    );

    server.abort();
}

/// A fake app that answers every request with a canned identity, so the typed read is exercised
/// over the real socket and framing.
async fn identity_server(listener: UnixListener, who: Whoami) {
    let (mut stream, _addr) = listener.accept().await.expect("accept");
    while let Some(_request) = read_frame::<_, IpcRequest>(&mut stream)
        .await
        .expect("read request")
    {
        let reply: IpcResult = Ok(IpcResponse::Whoami(who.clone()));
        write_frame(&mut stream, &reply).await.expect("write reply");
    }
}

#[tokio::test]
async fn whoami_reads_the_identity_the_app_resolved() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let who = Whoami {
        session: SessionId::from_raw(1),
        origin: Origin::Process(ProcessId::from_raw(7)),
        bound_process: None,
        provider: Some(AgentKind::Generic),
        selected_process: None,
        effective_project: None,
    };
    let server = tokio::spawn(identity_server(listener, who.clone()));

    let client = AppClient::new(None, socket);
    assert_eq!(client.whoami().await.expect("the app answers"), who);

    drop(client);
    server.abort();
}

/// A reply of the wrong shape is the app not keeping its side of the protocol, so it is a broken
/// connection rather than an identity the caller could act on.
#[tokio::test]
async fn whoami_treats_a_reply_of_the_wrong_shape_as_a_broken_connection() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("soloist-ipc.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let server = tokio::spawn(echo_server(listener));

    let client = AppClient::new(None, socket);
    let err = client.whoami().await.expect_err("Acked is not an identity");
    assert!(matches!(err, ClientError::Transport));

    drop(client);
    server.abort();
}
