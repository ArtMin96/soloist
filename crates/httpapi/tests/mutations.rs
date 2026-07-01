//! The HTTP API's mutation routes: the local-auth gate, CORS, the focus callback, and that
//! an authorized mutation reaches the real core and changes state. Driven end to end through
//! the router over a façade built from in-memory fakes — no real socket, so the assertions
//! are deterministic.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tower::ServiceExt;

use soloist_core::testing::{
    terminal_registration, FakeAgentToolRepo, FakeProjectRepo, FakeSpawner, FakeTrustRepo,
};
use soloist_core::{
    AgentTool, CorePorts, DomainEvent, Facade, ProcStatus, ProcessId, ProcessKind, ProjectId,
    TokioClock,
};
use soloist_httpapi::{router, ApiState, FocusFn};
use soloist_ipc::http::{
    SpawnResponse, LOCAL_AUTH_HEADER, LOCAL_AUTH_VALUE, STATUS_FORBIDDEN, STATUS_NOT_FOUND,
    STATUS_UNAUTHORIZED,
};

/// The header pair an authorized mutation carries.
const AUTH: (&str, &str) = (LOCAL_AUTH_HEADER, LOCAL_AUTH_VALUE);

/// A façade over fakes with one registered terminal — ungated, so `start` needs no trust —
/// returning the façade and the process id to target.
fn facade_with_terminal() -> (Arc<Facade>, ProcessId) {
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

/// A façade seeded with the built-in agent tools and one loaded (empty) project, returning the
/// façade, the project id, and the temp dir to keep alive — the setup `spawn-agent` needs.
fn facade_with_agent_tool() -> (Arc<Facade>, ProjectId, tempfile::TempDir) {
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .agent_tools(Arc::new(FakeAgentToolRepo::new(
            AgentTool::builtin_defaults(),
        )))
        .build(),
    ));
    let dir = tempfile::tempdir().expect("temp dir");
    let project = facade.load_project(dir.path()).expect("load project");
    (facade, project.id, dir)
}

/// A `POST` request to `uri` carrying each given header.
fn post(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::empty()).expect("request")
}

/// A `POST` request to `uri` with a JSON `body` and each given header.
fn post_json(uri: &str, headers: &[(&str, &str)], body: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::from(body.to_string())).expect("request")
}

/// Waits (bounded) for `id` to reach `target` on the event bus, so a state-changing mutation
/// is observed rather than assumed.
async fn await_status(
    events: &mut broadcast::Receiver<DomainEvent>,
    id: ProcessId,
    target: ProcStatus,
) {
    let wait = async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::ProcessStatusChanged {
                    id: changed, to, ..
                }) if changed == id && to == target => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("event bus closed"),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), wait)
        .await
        .expect("process reached the target status");
}

#[tokio::test]
async fn a_mutation_without_the_auth_header_is_rejected() {
    let (facade, id) = facade_with_terminal();
    let app = router(ApiState::new(facade));
    let response = app
        .oneshot(post(&format!("/processes/{}/start", id.get()), &[]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_mutation_with_a_wrong_auth_value_is_rejected() {
    let (facade, id) = facade_with_terminal();
    let app = router(ApiState::new(facade));
    let response = app
        .oneshot(post(
            &format!("/processes/{}/start", id.get()),
            &[(LOCAL_AUTH_HEADER, "0")],
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_read_route_stays_open_without_auth() {
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/processes")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_authorized_start_reaches_the_core_and_runs_the_process() {
    let (facade, id) = facade_with_terminal();
    let mut events = facade.subscribe();
    let app = router(ApiState::new(Arc::clone(&facade)));
    let response = app
        .oneshot(post(&format!("/processes/{}/start", id.get()), &[AUTH]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    await_status(&mut events, id, ProcStatus::Running).await;
}

#[tokio::test]
async fn restarting_an_unknown_process_maps_to_404() {
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade));
    let response = app
        .oneshot(post("/processes/999999/restart", &[AUTH]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_project_bulk_stop_is_authorized_and_succeeds() {
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade));
    let response = app
        .oneshot(post("/projects/1/stop-all", &[AUTH]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn focus_raises_the_window() {
    let (facade, _id) = facade_with_terminal();
    let raised = Arc::new(AtomicBool::new(false));
    let probe = Arc::clone(&raised);
    let focus: FocusFn = Arc::new(move || probe.store(true, Ordering::SeqCst));
    let app = router(ApiState::new(facade).with_focus(focus));
    let response = app
        .oneshot(post("/focus", &[AUTH]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        raised.load(Ordering::SeqCst),
        "POST /focus invokes the focus callback"
    );
}

#[tokio::test]
async fn focus_without_auth_is_rejected_before_the_handler_runs() {
    let (facade, _id) = facade_with_terminal();
    let raised = Arc::new(AtomicBool::new(false));
    let probe = Arc::clone(&raised);
    let focus: FocusFn = Arc::new(move || probe.store(true, Ordering::SeqCst));
    let app = router(ApiState::new(facade).with_focus(focus));
    let response = app.oneshot(post("/focus", &[])).await.expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        !raised.load(Ordering::SeqCst),
        "the gate rejects before the focus callback can fire"
    );
}

#[tokio::test]
async fn spawn_agent_launches_a_known_tool_and_returns_its_id() {
    let (facade, project, _dir) = facade_with_agent_tool();
    let app = router(ApiState::new(Arc::clone(&facade)));
    let response = app
        .oneshot(post_json(
            &format!("/projects/{}/spawn-agent", project.get()),
            &[AUTH],
            r#"{"tool":"Claude","args":[]}"#,
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let spawned: SpawnResponse = serde_json::from_slice(&body).expect("spawn response");
    // The returned id names a real, newly-registered Agent process in the stack.
    assert!(
        facade
            .snapshot()
            .iter()
            .any(|p| p.id.get() == spawned.id && p.kind == ProcessKind::Agent),
        "the spawned id is a registered Agent process"
    );
}

#[tokio::test]
async fn spawn_agent_with_an_unknown_tool_is_404() {
    let (facade, project, _dir) = facade_with_agent_tool();
    let app = router(ApiState::new(facade));
    let response = app
        .oneshot(post_json(
            &format!("/projects/{}/spawn-agent", project.get()),
            &[AUTH],
            r#"{"tool":"Nonexistent"}"#,
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn spawn_agent_without_auth_is_rejected() {
    let (facade, project, _dir) = facade_with_agent_tool();
    let app = router(ApiState::new(facade));
    let response = app
        .oneshot(post_json(
            &format!("/projects/{}/spawn-agent", project.get()),
            &[],
            r#"{"tool":"Claude"}"#,
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn the_shared_status_contract_matches_the_codes_the_server_returns() {
    // The CLI interprets these `ipc::http` constants; the server returns these axum codes.
    // Pinning them together keeps the two halves of the contract from drifting apart.
    assert_eq!(STATUS_UNAUTHORIZED, StatusCode::UNAUTHORIZED.as_u16());
    assert_eq!(STATUS_FORBIDDEN, StatusCode::FORBIDDEN.as_u16());
    assert_eq!(STATUS_NOT_FOUND, StatusCode::NOT_FOUND.as_u16());
}

#[tokio::test]
async fn a_non_loopback_origin_gets_no_cors_allowance_on_a_mutation() {
    let (facade, id) = facade_with_terminal();
    let app = router(ApiState::new(facade));
    let response = app
        .oneshot(post(
            &format!("/processes/{}/stop", id.get()),
            &[AUTH, ("origin", "https://evil.example")],
        ))
        .await
        .expect("response");
    assert!(response
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}
