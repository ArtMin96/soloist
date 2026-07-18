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
    terminal_registration, FakeAgentToolRepo, FakeProjectRepo, FakeScratchpadRepo, FakeSpawner,
    FakeTrustRepo,
};
use soloist_core::{
    AgentTool, CorePorts, DomainEvent, Facade, ProcStatus, ProcessId, ProcessKind, ProjectId,
    TokioClock,
};
use soloist_httpapi::{router, ApiState, FocusFn};
use soloist_ipc::http::{
    SpawnResponse, LOCAL_AUTH_HEADER, STATUS_FORBIDDEN, STATUS_NOT_FOUND, STATUS_UNAUTHORIZED,
};

/// The per-launch token these tests seed the server with and present on authorized requests.
const TEST_TOKEN: &str = "test-token";

/// The header pair an authorized mutation carries.
const AUTH: (&str, &str) = (LOCAL_AUTH_HEADER, TEST_TOKEN);

/// A loopback `Host`, as a real HTTP/1.1 client sends — every request needs one to pass the
/// `Host` guard.
const LOOPBACK_HOST: (&str, &str) = ("host", "127.0.0.1");

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

/// A façade with one loaded `solo.yml` command and an empty trust store, so the command is
/// untrusted — the setup the HTTP trust-gate (403) tests need. Returns the façade, the process
/// id to target, and the temp dir to keep alive for the test's duration.
fn facade_with_untrusted_command() -> (Arc<Facade>, u64, tempfile::TempDir) {
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ));
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("solo.yml"),
        "processes:\n  Web:\n    command: npm run dev\n",
    )
    .expect("write");
    facade.load_project(dir.path()).expect("load");
    let id = facade.snapshot()[0].id.get();
    (facade, id, dir)
}

/// A `GET` request to `uri` carrying a loopback `Host` plus each given header.
fn get(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder()
        .uri(uri)
        .header(LOOPBACK_HOST.0, LOOPBACK_HOST.1);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::empty()).expect("request")
}

/// A `POST` request to `uri` carrying a loopback `Host` plus each given header.
fn post(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header(LOOPBACK_HOST.0, LOOPBACK_HOST.1);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::empty()).expect("request")
}

/// A `POST` request to `uri` with a JSON `body`, a loopback `Host`, and each given header.
fn post_json(uri: &str, headers: &[(&str, &str)], body: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header(LOOPBACK_HOST.0, LOOPBACK_HOST.1)
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
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(post(&format!("/processes/{}/start", id.get()), &[]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_mutation_with_a_wrong_auth_value_is_rejected() {
    let (facade, id) = facade_with_terminal();
    let app = router(ApiState::new(facade, TEST_TOKEN));
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
async fn a_read_route_now_requires_the_token() {
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    // A loopback Host but no token: reads are no longer open, so this is rejected.
    let response = app.oneshot(get("/processes", &[])).await.expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_authorized_read_route_succeeds() {
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(get("/processes", &[AUTH]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn every_mutation_route_requires_the_token() {
    // One guard layer authenticates all mutations; this pins that every mutation route is actually
    // behind it, so a route accidentally added to the open read router would be caught. The token
    // layer rejects before the handler runs, so a placeholder id and an empty body suffice.
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade, TEST_TOKEN));

    let post_routes = [
        "/processes/1/start",
        "/processes/1/stop",
        "/processes/1/restart",
        "/projects/1/start-auto",
        "/projects/1/start-all",
        "/projects/1/stop-all",
        "/projects/1/restart-running",
        "/projects/1/restart-all",
        "/projects/1/reload",
        "/projects/1/spawn-agent",
        "/projects/1/transfer-todo",
        "/projects/1/transfer-scratchpad",
        "/focus",
    ];
    for path in post_routes {
        let response = app
            .clone()
            .oneshot(post(path, &[]))
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "POST {path} must require the token"
        );
    }
    // The one non-POST mutation.
    let response = app
        .clone()
        .oneshot(delete("/projects/1", &[]))
        .await
        .expect("response");
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "DELETE /projects/1 must require the token"
    );
}

#[tokio::test]
async fn starting_an_untrusted_command_is_403() {
    // A `solo.yml` command is trust-gated, and the fake trust store starts empty, so the command
    // is untrusted — its start must be refused over HTTP with a 403.
    let (facade, id, _dir) = facade_with_untrusted_command();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(post(&format!("/processes/{id}/start"), &[AUTH]))
        .await
        .expect("response");
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "an untrusted command's start is refused by the core trust gate"
    );
}

#[tokio::test]
async fn restarting_an_untrusted_command_is_403() {
    // The trust gate also covers restart over the adapter, not just start.
    let (facade, id, _dir) = facade_with_untrusted_command();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(post(&format!("/processes/{id}/restart"), &[AUTH]))
        .await
        .expect("response");
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "an untrusted command's restart is refused by the core trust gate"
    );
}

#[tokio::test]
async fn an_authorized_start_reaches_the_core_and_runs_the_process() {
    let (facade, id) = facade_with_terminal();
    let mut events = facade.subscribe();
    let app = router(ApiState::new(Arc::clone(&facade), TEST_TOKEN));
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
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(post("/processes/999999/restart", &[AUTH]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_project_bulk_stop_is_authorized_and_succeeds() {
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade, TEST_TOKEN));
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
    let app = router(ApiState::new(facade, TEST_TOKEN).with_focus(focus));
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
    let app = router(ApiState::new(facade, TEST_TOKEN).with_focus(focus));
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
    let app = router(ApiState::new(Arc::clone(&facade), TEST_TOKEN));
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
async fn spawn_agent_is_rate_limited_after_a_burst() {
    // A same-user caller is already authenticated, but nothing stopped a runaway loop from
    // spawning agent processes without bound. After the per-window cap, further spawns are 429.
    let (facade, project, _dir) = facade_with_agent_tool();
    let app = router(ApiState::new(Arc::clone(&facade), TEST_TOKEN));
    let mut last = StatusCode::OK;
    for _ in 0..40 {
        last = app
            .clone()
            .oneshot(post_json(
                &format!("/projects/{}/spawn-agent", project.get()),
                &[AUTH],
                r#"{"tool":"Claude","args":[]}"#,
            ))
            .await
            .expect("response")
            .status();
        if last == StatusCode::TOO_MANY_REQUESTS {
            break;
        }
    }
    assert_eq!(
        last,
        StatusCode::TOO_MANY_REQUESTS,
        "the spawn burst is eventually refused by the rate cap"
    );
}

#[tokio::test]
async fn spawn_agent_with_an_unknown_tool_is_404() {
    let (facade, project, _dir) = facade_with_agent_tool();
    let app = router(ApiState::new(facade, TEST_TOKEN));
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
    let app = router(ApiState::new(facade, TEST_TOKEN));
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

#[tokio::test]
async fn reload_reconciles_a_changed_solo_yml() {
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ));
    let dir = tempfile::tempdir().expect("temp dir");
    let config = dir.path().join("solo.yml");
    std::fs::write(&config, "processes:\n  Web:\n    command: npm run dev\n").expect("write");
    let project = facade.load_project(dir.path()).expect("load");
    assert_eq!(facade.snapshot().len(), 1);

    // Add a command on disk, then reload over HTTP: the reconcile registers it without
    // duplicating the existing command.
    std::fs::write(
        &config,
        "processes:\n  Web:\n    command: npm run dev\n  Api:\n    command: cargo run\n",
    )
    .expect("write");
    let app = router(ApiState::new(Arc::clone(&facade), TEST_TOKEN));
    let response = app
        .oneshot(post(
            &format!("/projects/{}/reload", project.id.get()),
            &[AUTH],
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(facade.snapshot().len(), 2);
}

#[tokio::test]
async fn reload_of_an_unknown_project_is_404() {
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(post("/projects/999999/reload", &[AUTH]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// A `DELETE` request to `uri` carrying a loopback `Host` plus each given header.
fn delete(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder()
        .method("DELETE")
        .uri(uri)
        .header(LOOPBACK_HOST.0, LOOPBACK_HOST.1);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::empty()).expect("request")
}

#[tokio::test]
async fn removing_a_project_closes_its_processes_and_deletes_it() {
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ));
    let dir = tempfile::tempdir().expect("temp dir");
    let config = dir.path().join("solo.yml");
    std::fs::write(&config, "processes:\n  Web:\n    command: npm run dev\n").expect("write");
    let project = facade.load_project(dir.path()).expect("load");
    assert_eq!(facade.snapshot().len(), 1);

    let app = router(ApiState::new(Arc::clone(&facade), TEST_TOKEN));
    let response = app
        .oneshot(delete(&format!("/projects/{}", project.id.get()), &[AUTH]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    // The registration set and the project read model are both empty; the user's file remains.
    assert!(facade.snapshot().is_empty());
    assert!(facade.projects_snapshot().expect("snapshot").is_empty());
    assert!(config.exists(), "removal never touches disk");
}

#[tokio::test]
async fn removing_a_project_without_auth_is_rejected() {
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ));
    let dir = tempfile::tempdir().expect("temp dir");
    let project = facade.load_project(dir.path()).expect("load");

    let app = router(ApiState::new(Arc::clone(&facade), TEST_TOKEN));
    let response = app
        .oneshot(delete(&format!("/projects/{}", project.id.get()), &[]))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        facade.projects_snapshot().expect("snapshot").len(),
        1,
        "an unauthorized delete removes nothing"
    );
}

#[tokio::test]
async fn removing_an_unknown_project_is_404() {
    let (facade, _id) = facade_with_terminal();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(delete("/projects/999999", &[AUTH]))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// A façade with the scratchpad store wired and two empty projects loaded, returning the façade,
/// both project ids, and the temp dirs to keep alive — the setup the transfer tests share.
fn facade_with_two_projects() -> (
    Arc<Facade>,
    ProjectId,
    ProjectId,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .scratchpad_repo(Arc::new(FakeScratchpadRepo::new()))
        .build(),
    ));
    let a_dir = tempfile::tempdir().expect("temp dir a");
    let b_dir = tempfile::tempdir().expect("temp dir b");
    let a = facade.load_project(a_dir.path()).expect("load a").id;
    let b = facade.load_project(b_dir.path()).expect("load b").id;
    (facade, a, b, a_dir, b_dir)
}

/// A representative scratchpad Markdown body to seed a transfer test with.
fn scratchpad_body() -> String {
    "## Objective\nShip v1\n\n## Status\nin progress".to_owned()
}

#[tokio::test]
async fn transfer_scratchpad_moves_it_to_the_target_project() {
    let (facade, a, b, _a_dir, _b_dir) = facade_with_two_projects();
    facade
        .scratchpad_write_in(a, "plan", scratchpad_body(), None)
        .expect("seed in A");
    let app = router(ApiState::new(Arc::clone(&facade), TEST_TOKEN));
    let response = app
        .oneshot(post_json(
            &format!("/projects/{}/transfer-scratchpad", a.get()),
            &[AUTH],
            &format!(r#"{{"name":"plan","to_project":{}}}"#, b.get()),
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(facade.scratchpad_read_in(b, "plan").is_ok(), "now in B");
    assert!(facade.scratchpad_read_in(a, "plan").is_err(), "gone from A");
}

#[tokio::test]
async fn transfer_scratchpad_to_an_unknown_target_is_404_and_does_not_orphan() {
    let (facade, a, _b, _a_dir, _b_dir) = facade_with_two_projects();
    facade
        .scratchpad_write_in(a, "plan", scratchpad_body(), None)
        .expect("seed in A");
    let app = router(ApiState::new(Arc::clone(&facade), TEST_TOKEN));
    let response = app
        .oneshot(post_json(
            &format!("/projects/{}/transfer-scratchpad", a.get()),
            &[AUTH],
            r#"{"name":"plan","to_project":999999}"#,
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        facade.scratchpad_read_in(a, "plan").is_ok(),
        "still in A — a bad target never orphans it"
    );
}

#[tokio::test]
async fn transfer_scratchpad_without_auth_is_rejected() {
    let (facade, a, b, _a_dir, _b_dir) = facade_with_two_projects();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(post_json(
            &format!("/projects/{}/transfer-scratchpad", a.get()),
            &[],
            &format!(r#"{{"name":"plan","to_project":{}}}"#, b.get()),
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn transfer_todo_with_an_unknown_todo_is_404() {
    let (facade, a, b, _a_dir, _b_dir) = facade_with_two_projects();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    // The target project exists, but there is no such todo in the source → UnknownTodo → 404.
    let response = app
        .oneshot(post_json(
            &format!("/projects/{}/transfer-todo", a.get()),
            &[AUTH],
            &format!(r#"{{"todo":9999,"to_project":{}}}"#, b.get()),
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn transfer_todo_without_auth_is_rejected() {
    let (facade, a, b, _a_dir, _b_dir) = facade_with_two_projects();
    let app = router(ApiState::new(facade, TEST_TOKEN));
    let response = app
        .oneshot(post_json(
            &format!("/projects/{}/transfer-todo", a.get()),
            &[],
            &format!(r#"{{"todo":1,"to_project":{}}}"#, b.get()),
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
    let app = router(ApiState::new(facade, TEST_TOKEN));
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
