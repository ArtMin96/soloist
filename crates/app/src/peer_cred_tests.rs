use super::{peer_pgid, peer_uid_permitted};
use nix::unistd::getpgrp;
use tokio::net::{UnixListener, UnixStream};

/// Only the UID Soloist runs as may authenticate a session; any other peer is refused. The
/// `0700` data dir already confines the socket to this user, so this is fail-closed
/// defense-in-depth for the case where that ever does not hold.
#[test]
fn only_the_apps_own_uid_may_authenticate_a_session() {
    let own = 1000;
    assert!(peer_uid_permitted(own, own), "the same uid is served");
    assert!(
        !peer_uid_permitted(1001, own),
        "a peer from a different uid is refused"
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
