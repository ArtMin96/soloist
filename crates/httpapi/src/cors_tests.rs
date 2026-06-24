use super::is_localhost;
use axum::http::HeaderValue;

fn allows(origin: &str) -> bool {
    is_localhost(&HeaderValue::from_str(origin).expect("valid header value"))
}

#[test]
fn loopback_origins_are_allowed() {
    assert!(allows("http://localhost"));
    assert!(allows("http://localhost:1420"));
    assert!(allows("https://localhost:5173"));
    assert!(allows("http://127.0.0.1:3000"));
    assert!(allows("http://[::1]:8080"));
}

#[test]
fn non_loopback_origins_are_rejected() {
    assert!(!allows("https://example.com"));
    assert!(!allows("http://evil.localhost.example.com"));
    assert!(!allows("http://10.0.0.5:3000"));
    assert!(!allows("not-an-origin"));
}
