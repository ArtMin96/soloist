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

use soloist_core::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{CorePorts, DomainEvent, Facade, ProcStatus, ProcessId, ProjectId, TokioClock};
use soloist_httpapi::{router, ApiState, FocusFn};
use soloist_ipc::http::{
    LOCAL_AUTH_HEADER, LOCAL_AUTH_VALUE, STATUS_FORBIDDEN, STATUS_NOT_FOUND, STATUS_UNAUTHORIZED,
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

/// A `POST` request to `uri` carrying each given header.
fn post(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::empty()).expect("request")
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
