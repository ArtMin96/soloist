//! Whether a host names the loopback interface — the one rule the `Host`-header guard and
//! the CORS origin check both consult, so "is this local?" is decided in a single place.

/// The host names that resolve to the loopback interface. A request addressed to anything
/// else is not talking to *this* server by its real address, so it is refused.
const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

/// Whether `authority` (a `host[:port]`, e.g. a `Host` header or the authority half of an
/// `Origin`) names the loopback interface. Handles the bracketed IPv6 form (`[::1]:port`)
/// and a bare host with no port; an empty or unparseable authority is not loopback.
pub(crate) fn host_is_loopback(authority: &str) -> bool {
    let host = match authority.strip_prefix('[') {
        // Bracketed IPv6: the host is everything up to the closing bracket.
        Some(rest) => rest.split_once(']').map(|(host, _)| host).unwrap_or(rest),
        None => authority.split(':').next().unwrap_or(authority),
    };
    LOOPBACK_HOSTS.contains(&host)
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
