//! Reading the connecting peer's credentials from a Unix-socket connection.
//!
//! The local IPC server authenticates a session's project scope against the kernel-reported
//! credentials of the process on the other end of the socket (`SO_PEERCRED`), resolving that peer's
//! process **group** so the core can match it to the managed process the caller runs in, and its
//! working **directory** so the core can match it to the project root the caller runs under. This
//! is the one place that OS credential detail lives; the core only ever compares plain
//! process-group ids and paths it is handed.

use std::path::PathBuf;

use nix::unistd::{getpgid, Pid, Uid};
use soloist_core::PeerCredentials;
use tokio::net::UnixStream;

/// The connecting peer's [`PeerCredentials`] — its process group and working directory, each
/// `None` when it cannot be resolved (the peer reported no pid, exited before we looked, or its
/// `/proc` entry was unreadable). Both `None` leaves the session unauthenticated: it can use the
/// open read tools but cannot bind to a process or select a project scope (both require a matching
/// home process or directory), so no cross-project surface is granted. The pid (from `SO_PEERCRED`)
/// and its group/cwd are read in two steps: if the peer exits and the kernel reuses its pid between
/// the two reads, the resolved group/cwd describe the replacement process instead — a narrow (two
/// adjacent syscalls), same-UID (`0700` socket) window in which a recycled pid could resolve to a
/// different open project. An absent fact still grants no scope (fail closed), but this is a real
/// residual of reading the group/cwd from `/proc` by pid rather than from a handle pinned to the
/// original peer; it is not closed here.
///
/// Returns an error — which the caller treats as a dead connection and drops — when the peer
/// credentials cannot be read at all, **or** when the peer is a different UID than Soloist runs as.
/// The `0700` data directory already confines the socket to the owning user, so a foreign UID should
/// never reach here; asserting it anyway (`SO_PEERCRED` reports the peer's UID unforgeably) fails the
/// connection closed rather than serving any surface to another user.
pub fn peer_credentials(stream: &UnixStream) -> std::io::Result<PeerCredentials> {
    let cred = stream.peer_cred()?;
    let own = Uid::current().as_raw();
    if !peer_uid_permitted(cred.uid(), own) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("peer uid {} is not Soloist's uid {own}", cred.uid()),
        ));
    }
    let Some(pid) = cred.pid() else {
        return Ok(PeerCredentials::default());
    };
    Ok(PeerCredentials {
        pgid: getpgid(Some(Pid::from_raw(pid)))
            .ok()
            .map(|pgid| pgid.as_raw()),
        cwd: peer_cwd(pid),
    })
}

/// The peer's working directory, read from `/proc/<pid>/cwd` (a symlink to the resolved absolute
/// path). `None` when it cannot be read — the peer exited, or the kernel denied the read — so an
/// unreadable directory grants no scope rather than a wrong one (fail closed). The read is confined
/// here so the core is handed a plain path, never a `/proc` detail.
fn peer_cwd(pid: i32) -> Option<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

/// Whether a peer connecting as `peer_uid` may be served on a socket owned by `own_uid`: only
/// the app's own UID. Split out as a pure decision so the fail-closed rule is unit-tested
/// without a real cross-UID socket.
fn peer_uid_permitted(peer_uid: u32, own_uid: u32) -> bool {
    peer_uid == own_uid
}

/// What to do with a new connection given the resolved peer credentials from [`peer_credentials`]:
/// open a session with them, or drop the connection. Credentials with both facts `None` are
/// *unauthenticated* (open read tools only, no bind or project select) — that is **not** a drop;
/// only refused credentials (unreadable, or a foreign UID, surfaced as an `Err`) drop the
/// connection. Split out as a pure mapping so this fail-closed decision is unit-tested directly,
/// without a real broken socket.
#[derive(Debug, PartialEq, Eq)]
pub enum PeerScope {
    /// Open a session with these peer credentials; both facts `None` is unauthenticated.
    Open(PeerCredentials),
    /// Refuse the connection outright.
    Drop,
}

pub fn peer_scope(resolved: &std::io::Result<PeerCredentials>) -> PeerScope {
    match resolved {
        Ok(credentials) => PeerScope::Open(credentials.clone()),
        Err(_) => PeerScope::Drop,
    }
}

#[cfg(test)]
#[path = "peer_cred_tests.rs"]
mod tests;
