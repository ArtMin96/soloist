//! The ask half of the trust workflow meeting the do half: a caller requests trust for a command
//! variant, the user approves exactly what was shown, and the same variant is then spawnable.

use super::*;

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::facade::{Facade, SpawnProcessRequest};
use crate::ids::{ProjectId, SessionId};
use crate::testing::{agent_registration, bound_session, facade_with_agent_tool, TEST_PEER_PGID};

const COMMAND: &str = "npm run build";
const REASON: &str = "the release build needs it";

/// The caller-supplied fields the trust variant digests: `working_dir` and `env`, carried
/// through as written on both sides of the round trip.
fn env() -> BTreeMap<String, String> {
    BTreeMap::from([("RELEASE_CHANNEL".to_string(), "beta".to_string())])
}

/// What the caller asks the user to approve.
fn ask() -> CommandTrustRequest {
    CommandTrustRequest {
        command: COMMAND.into(),
        working_dir: Some(PathBuf::from("web")),
        env: env(),
        label: None,
        reason: REASON.into(),
    }
}

/// The same caller fields, offered again to `spawn_process` once the ask above was approved.
fn spawn_request() -> SpawnProcessRequest {
    SpawnProcessRequest {
        command: COMMAND.into(),
        working_dir: Some(PathBuf::from("web")),
        env: env(),
        label: None,
    }
}

/// A façade with one project and a session bound to a live agent in it — the caller a real ask,
/// then a real spawn, both come from.
fn facade_with_agent_caller() -> (Facade, ProjectId, SessionId) {
    let (facade, project) = facade_with_agent_tool();
    let agent = facade
        .supervisor()
        .register(agent_registration(project, "lead"));
    let session = bound_session(&facade, agent, TEST_PEER_PGID);
    (facade, project, session)
}

#[tokio::test]
async fn approving_a_requested_variant_permits_the_matching_spawn() {
    let (facade, project, session) = facade_with_agent_caller();

    facade
        .scoped(session)
        .request_command_trust(ask())
        .expect("record the request");
    let mut pending = facade.pending_trust_requests(project);
    assert_eq!(pending.len(), 1, "expected exactly one open request");
    let request = pending.remove(0);
    facade
        .approve_trust_request(request.id, &request.review.variant_hash)
        .expect("approve exactly the variant that was displayed");

    facade
        .scoped(session)
        .spawn_process(spawn_request())
        .expect("the variant the user just approved must be startable");
}
