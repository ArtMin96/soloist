//! The HTTP API's read routes and CORS policy, driven end to end through the router over
//! a façade built from in-memory fakes — no real socket, so the assertions are
//! deterministic.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use soloist_core::testing::{terminal_registration, FakeProjectRepo, FakeSpawner, FakeTrustRepo};
use soloist_core::{CorePorts, Facade, ProjectId, TokioClock};
use soloist_httpapi::{router, ApiState};

/// A façade over fakes with one registered (resting) terminal, so the read routes have a
/// real row to project.
fn facade_with_one_process() -> Arc<Facade> {
    let facade = Arc::new(Facade::new(
        CorePorts::builder(
            Arc::new(FakeSpawner::exits_on_terminate()),
            Arc::new(TokioClock),
            Arc::new(FakeTrustRepo::new()),
            Arc::new(FakeProjectRepo::new()),
        )
        .build(),
    ));
    facade.supervisor().register(terminal_registration(
        ProjectId::from_raw(1),
        "web",
        "sleep 1",
    ));
    facade
}

async fn get(app: axum::Router, uri: &str, origin: Option<&str>) -> axum::http::Response<Body> {
    let mut builder = Request::builder().uri(uri);
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    app.oneshot(builder.body(Body::empty()).expect("request"))
        .await
        .expect("response")
}

async fn json(response: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json body")
}

#[tokio::test]
async fn health_reports_ok_and_a_version() {
    let app = router(ApiState::new(facade_with_one_process()));
    let response = get(app, "/health", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["ok"], true);
    assert!(body["version"].as_str().is_some_and(|v| !v.is_empty()));
}

#[tokio::test]
async fn processes_returns_the_live_read_model_as_json() {
    let app = router(ApiState::new(facade_with_one_process()));
    let body = json(get(app, "/processes", None).await).await;
    let rows = body.as_array().expect("array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["label"], "web");
    assert_eq!(rows[0]["kind"], "Terminal");
}

#[tokio::test]
async fn output_returns_a_line_array_and_an_unknown_id_reads_as_empty() {
    let facade = facade_with_one_process();
    let id = facade.snapshot()[0].id.get();
    let app = router(ApiState::new(facade));

    // A known (resting) process: 200 with a JSON array — empty, since it never started.
    let response = get(app.clone(), &format!("/processes/{id}/output"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(json(response).await.as_array().expect("array").is_empty());

    // The `lines` query cap is accepted and still yields an array.
    let response = get(
        app.clone(),
        &format!("/processes/{id}/output?lines=5"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(json(response).await.is_array());

    // An unknown id has no buffer and reads as an empty list rather than erroring (like ports).
    let response = get(app, "/processes/999999/output", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(json(response).await.as_array().expect("array").is_empty());
}

#[tokio::test]
async fn status_summarizes_projects_and_processes() {
    let app = router(ApiState::new(facade_with_one_process()));
    let body = json(get(app, "/status", None).await).await;
    assert_eq!(body["processes"], 1);
    assert_eq!(body["running"], 0);
}

#[tokio::test]
async fn a_loopback_origin_is_allowed_by_cors() {
    let app = router(ApiState::new(facade_with_one_process()));
    let response = get(app, "/processes", Some("http://localhost:1420")).await;
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .expect("allow-origin header"),
        "http://localhost:1420"
    );
}

#[tokio::test]
async fn a_non_loopback_origin_is_refused_by_cors() {
    let app = router(ApiState::new(facade_with_one_process()));
    let response = get(app, "/processes", Some("https://evil.example")).await;
    assert!(response
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}
