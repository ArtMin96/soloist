//! Authenticating a test session to a process the way the UDS adapter authenticates an MCP
//! client from its peer credentials — without spinning up a real PTY. The single source for
//! the synthetic peer group and the bind/select scope setup the identity tests share, across
//! the core's own tests and the adapter crates that consume the `testing` feature.

use crate::facade::Facade;
use crate::ids::{ProcessId, SessionId};

/// A synthetic peer process group for a session authenticated to its bound process. Any
/// value works: each test builds its own facade and assigns this group to the process it
/// scopes to, so the façade's bind/select authenticity check matches.
pub const TEST_PEER_PGID: i32 = 5000;

/// Opens an identity session authenticated to `process`, as the UDS adapter would for an MCP
/// client running inside that process's group: it assigns the process a synthetic live group
/// (standing in for the group a real spawn creates) and opens a session whose peer shares it,
/// so a later [`bind_session_process`](Facade::bind_session_process) to that process — or a
/// [`select_project`](Facade::select_project) of its project — passes the façade's
/// authenticity check. Does not bind; a caller that needs the bound origin binds itself.
pub fn authentic_session(facade: &Facade, process: ProcessId, pgid: i32) -> SessionId {
    facade.supervisor().assign_test_group(process, pgid);
    facade.open_session(Some(pgid))
}
