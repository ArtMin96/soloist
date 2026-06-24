//! The mutation auth gate.
//!
//! Loopback bind plus localhost CORS keep remote and cross-origin callers out; this header
//! is the deliberate, weak local gate that stops a drive-by request from a page the user
//! merely happens to be viewing. Applied via `route_layer` to the mutation sub-router only,
//! so read routes stay open on loopback.

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;

use soloist_ipc::http::{LOCAL_AUTH_HEADER, LOCAL_AUTH_VALUE};

/// Admits a mutating request only when it carries `LOCAL_AUTH_HEADER: LOCAL_AUTH_VALUE`,
/// otherwise rejects it with `401 Unauthorized` before the handler runs. The header name is
/// matched case-insensitively by the `http` crate; the value must match exactly.
pub async fn require_local_auth(request: Request, next: Next) -> Result<Response, StatusCode> {
    let authorized = request
        .headers()
        .get(LOCAL_AUTH_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == LOCAL_AUTH_VALUE);
    if authorized {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
