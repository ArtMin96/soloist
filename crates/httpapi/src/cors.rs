//! The localhost-only CORS policy for the HTTP API. A browser page may call the API only
//! when it is itself served from `localhost`/`127.0.0.1`/`[::1]`, so a page on the wider
//! web the user happens to be viewing cannot reach the loopback server from script.

use axum::http::{header, HeaderName, HeaderValue, Method};
use soloist_ipc::http::LOCAL_AUTH_HEADER;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::host::host_is_loopback;

/// A CORS layer that allows only loopback origins (any scheme, any port), the methods the
/// API uses, and the local-auth header — so cross-origin browser access is confined to
/// pages the user is running locally.
pub fn localhost_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _parts| {
            is_localhost(origin)
        }))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([
            header::CONTENT_TYPE,
            HeaderName::from_static(LOCAL_AUTH_HEADER),
        ])
}

/// Whether an `Origin` header names a loopback host. Parses `scheme://host[:port][/...]`
/// down to its authority, then defers to the shared [`host_is_loopback`] rule.
fn is_localhost(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Some((_scheme, authority)) = origin.split_once("://") else {
        return false;
    };
    let authority = authority.split('/').next().unwrap_or(authority);
    host_is_loopback(authority)
}

#[cfg(test)]
#[path = "cors_tests.rs"]
mod tests;
