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
