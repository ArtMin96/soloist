//! The two request guards every route passes through: a per-launch token check and a
//! `Host`-header check.
//!
//! The API binds a loopback TCP port, which any local user can reach and CORS never
//! constrains (CORS only governs browser script). So the token — a fresh secret each launch,
//! readable only from the owner's `0700` data directory — is the real boundary between
//! users, and the `Host` guard closes the DNS-rebinding path where a page the user is
//! viewing resolves its own domain to `127.0.0.1` to talk to this server as same-origin.

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use subtle::ConstantTimeEq;

use soloist_ipc::http::LOCAL_AUTH_HEADER;

use crate::host::host_is_loopback;
use crate::state::ApiState;

/// Admits a request only when it carries [`LOCAL_AUTH_HEADER`] equal to the running server's
/// per-launch token, otherwise rejects it with `401 Unauthorized` before the handler runs.
/// The comparison is constant-time: the token is a secret and the port is reachable by any
/// local user, so a byte-by-byte early return would leak it under timing analysis. A missing
/// header is a zero-length value, which never matches the fixed-length token.
pub async fn require_token(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let presented = request
        .headers()
        .get(LOCAL_AUTH_HEADER)
        .map(|value| value.as_bytes())
        .unwrap_or_default();
    if bool::from(presented.ct_eq(state.token().as_bytes())) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Admits a request only when its `Host` header names the loopback interface, otherwise
/// rejects it with `403 Forbidden`. A foreign or absent `Host` means the request reached us
/// under someone else's name — the shape of a DNS-rebinding attack — so it is refused before
/// the token is even considered.
pub async fn require_local_host(request: Request, next: Next) -> Result<Response, StatusCode> {
    let ok = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(host_is_loopback);
    if ok {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}
