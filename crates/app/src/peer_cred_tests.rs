use super::{peer_credentials, peer_scope, peer_uid_permitted, PeerScope};
use nix::unistd::getpgrp;
use soloist_core::PeerCredentials;
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
/// while any resolved credentials open a session — even fully unauthenticated ones (both facts
/// `None`) are downgraded to an unauthenticated scope, deliberately not dropped.
#[test]
fn the_connection_policy_drops_only_refused_credentials() {
    let resolved = PeerCredentials::in_group(42);
    assert_eq!(
        peer_scope(&Ok(resolved.clone())),
        PeerScope::Open(resolved),
        "a resolved peer is scoped to its credentials"
    );
    assert_eq!(
        peer_scope(&Ok(PeerCredentials::default())),
        PeerScope::Open(PeerCredentials::default()),
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

/// Resolving the peer of a connection this process opened yields this process's own group *and*
/// working directory: the test process is the one that called `connect`, so `SO_PEERCRED` reports
/// its pid, from which the group resolves to `getpgrp()` and the directory to this process's cwd.
/// This is exactly how a Soloist-launched agent's `soloist-mcp` child resolves to the agent's
/// managed process group, and how an externally-launched one resolves to the project directory it
/// runs in, in production.
#[tokio::test]
async fn resolves_the_connecting_peers_group_and_working_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("peer.sock");
    let listener = UnixListener::bind(&path).expect("bind");
    let _client = UnixStream::connect(&path).await.expect("connect");
    let (server, _addr) = listener.accept().await.expect("accept");

    let resolved = peer_credentials(&server).expect("read peer credentials");
    assert_eq!(
        resolved.pgid,
        Some(getpgrp().as_raw()),
        "the peer group is this process's own group"
    );
    let own_cwd =
        std::fs::canonicalize(std::env::current_dir().expect("cwd")).expect("canonical cwd");
    assert_eq!(
        resolved.cwd,
        Some(own_cwd),
        "the peer working directory is this process's own cwd, read from /proc"
    );
}
