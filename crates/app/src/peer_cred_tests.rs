use super::peer_pgid;
use nix::unistd::getpgrp;
use tokio::net::{UnixListener, UnixStream};

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
