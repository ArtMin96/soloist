use super::{peer_pgid, peer_scope, peer_uid_permitted, PeerScope};
use nix::unistd::getpgrp;
use tokio::net::{UnixListener, UnixStream};

/// Only the UID Soloist runs as may authenticate a session; every other peer is refused. The
/// `0700` data dir already confines the socket to this user, so this is fail-closed
/// defense-in-depth for the case where that ever does not hold.
#[test]
fn only_the_apps_own_uid_may_authenticate_a_session() {
    let own = 1000;
    assert!(peer_uid_permitted(own, own), "the same uid is served");
    // The gate is an exact match on the owning uid — there is no privileged (root) or off-by-one
    // bypass, so every other uid is refused.
    for foreign in [0, 1, own - 1, own + 1, u32::MAX] {
        assert!(
            !peer_uid_permitted(foreign, own),
            "a peer from uid {foreign} is refused on a socket owned by {own}"
        );
    }
}

/// The connection policy is fail-closed: refused credentials (an `Err`) drop the connection,
/// while a resolved or unresolved peer opens a session — an unresolved (`None`) peer is
/// downgraded to an unauthenticated scope, deliberately not dropped.
#[test]
fn the_connection_policy_drops_only_refused_credentials() {
    assert_eq!(
        peer_scope(&Ok(Some(42))),
        PeerScope::Open(Some(42)),
        "a resolved peer is scoped to its group"
    );
    assert_eq!(
        peer_scope(&Ok(None)),
        PeerScope::Open(None),
        "an unresolved peer opens an unauthenticated session, not a dropped connection"
    );
    let refused = Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "foreign uid",
    ));
    assert_eq!(
        peer_scope(&refused),
        PeerScope::Drop,
        "refused credentials drop the connection"
    );
}

/// Resolving the peer of a connection this process opened yields this process's own group:
/// the test process is the one that called `connect`, so `SO_PEERCRED` reports its pid and the
/// resolved group is `getpgrp()`. This is exactly how a Soloist-launched agent's `soloist-mcp`
/// child resolves to the agent's managed process group in production.
#[tokio::test]
async fn resolves_the_connecting_peers_process_group() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("peer.sock");
    let listener = UnixListener::bind(&path).expect("bind");
    let _client = UnixStream::connect(&path).await.expect("connect");
    let (server, _addr) = listener.accept().await.expect("accept");

    let resolved = peer_pgid(&server).expect("read peer credentials");
    assert_eq!(
        resolved,
        Some(getpgrp().as_raw()),
        "the peer group is this process's own group"
    );
}
