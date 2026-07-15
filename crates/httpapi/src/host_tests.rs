use super::host_is_loopback;

#[test]
fn loopback_hosts_with_and_without_a_port_pass() {
    for authority in [
        "localhost",
        "localhost:24678",
        "127.0.0.1",
        "127.0.0.1:24678",
        "[::1]",
        "[::1]:24678",
    ] {
        assert!(host_is_loopback(authority), "{authority} is loopback");
    }
}

#[test]
fn a_foreign_or_empty_host_is_not_loopback() {
    for authority in [
        "evil.example",
        "evil.example:24678",
        "10.0.0.5",
        "",
        "127.0.0.1.evil.example",
    ] {
        assert!(!host_is_loopback(authority), "{authority} is not loopback");
    }
}
