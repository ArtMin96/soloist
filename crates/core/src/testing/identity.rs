//! Authenticating a test session to a process the way the UDS adapter authenticates an MCP
//! client from its peer credentials — without spinning up a real PTY. The single source for
//! the synthetic peer group and the bind/select scope setup the identity tests share, across
//! the core's own tests and the adapter crates that consume the `testing` feature.

use std::path::PathBuf;

use crate::facade::Facade;
use crate::identity::PeerCredentials;
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
    facade.open_session(PeerCredentials::in_group(pgid))
}

/// Registers an agent named `label` in `project` and binds it to its own authenticated session —
/// the setup every mailbox operation needs, since each is authorized from the caller's bound
/// process. Each agent in one test needs its own `pgid`. Compiled for the core's own tests only —
/// it asserts via `expect`, which the core denies outside test builds.
#[cfg(test)]
pub fn bound_agent(
    facade: &Facade,
    project: crate::ids::ProjectId,
    label: &str,
    pgid: i32,
) -> (ProcessId, SessionId) {
    let process = facade
        .supervisor()
        .register(crate::testing::agent_registration(project, label));
    let session = authentic_session(facade, process, pgid);
    facade
        .scoped(session)
        .bind_session_process(process)
        .expect("bind the agent to its own process");
    (process, session)
}

/// Opens an identity session authenticated by its working directory, as the UDS adapter would for
/// an agent Soloist did not launch (its group owns no managed process): the session's peer reports
/// `cwd` and no group, so its effective scope — and a [`select_project`](Facade::select_project) of
/// the containing project — resolves from the project root that contains `cwd`. The single source
/// for the directory-authenticated setup the scope tests share.
pub fn session_in_dir(facade: &Facade, cwd: PathBuf) -> SessionId {
    facade.open_session(PeerCredentials::in_dir(cwd))
}
