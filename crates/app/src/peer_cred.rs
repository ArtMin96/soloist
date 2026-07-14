//! Reading the connecting peer's process group from a Unix-socket connection.
//!
//! The local IPC server authenticates a session's project scope against the kernel-reported
//! credentials of the process on the other end of the socket (`SO_PEERCRED`), resolving that
//! peer's process group so the core can match it to the managed process the caller runs in.
//! This is the one place that OS credential detail lives; the core only ever compares plain
//! process-group ids it is handed.

use nix::unistd::{getpgid, Pid, Uid};
use tokio::net::UnixStream;

/// The connecting peer's process group, or `None` when it cannot be resolved — the peer
/// reported no pid, or it exited before we looked. A `None` peer leaves the session
/// unauthenticated: it can use the open read tools but cannot bind to a process or select a
/// project scope (both require a matching home process), so no cross-project surface is
/// granted. The pid (from `SO_PEERCRED`) and its group are read in two steps; in the rare
/// case the peer exits and its pid is reused in between, the resolved group is stale and
/// matches no managed process — a refused bind, never a wrong-scope grant (fail closed).
///
/// Returns an error — which the caller treats as a dead connection and drops — when the peer
/// credentials cannot be read at all, **or** when the peer is a different UID than Soloist
/// runs as. The `0700` data directory already confines the socket to the owning user, so a
/// foreign UID should never reach here; asserting it anyway (`SO_PEERCRED` reports the peer's
/// UID unforgeably) fails the connection closed rather than serving any surface to another user.
pub fn peer_pgid(stream: &UnixStream) -> std::io::Result<Option<i32>> {
    let cred = stream.peer_cred()?;
    let own = Uid::current().as_raw();
    if !peer_uid_permitted(cred.uid(), own) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("peer uid {} is not Soloist's uid {own}", cred.uid()),
        ));
    }
    let Some(pid) = cred.pid() else {
        return Ok(None);
    };
    Ok(getpgid(Some(Pid::from_raw(pid)))
        .ok()
        .map(|pgid| pgid.as_raw()))
}

/// Whether a peer connecting as `peer_uid` may be served on a socket owned by `own_uid`: only
/// the app's own UID. Split out as a pure decision so the fail-closed rule is unit-tested
/// without a real cross-UID socket.
fn peer_uid_permitted(peer_uid: u32, own_uid: u32) -> bool {
    peer_uid == own_uid
}

#[cfg(test)]
#[path = "peer_cred_tests.rs"]
mod tests;
