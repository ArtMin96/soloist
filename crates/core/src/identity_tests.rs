use super::*;

#[test]
fn a_fresh_session_is_unbound_with_no_selection() {
    let identity = Identity::new();
    let session = identity.open(PeerCredentials::unauthenticated());
    assert_eq!(identity.origin(session), Origin::Unbound);
    assert_eq!(identity.selected_project(session), None);
    assert_eq!(identity.selected_process(session), None);
}

#[test]
fn open_mints_distinct_sessions() {
    let identity = Identity::new();
    assert_ne!(
        identity.open(PeerCredentials::unauthenticated()),
        identity.open(PeerCredentials::unauthenticated())
    );
}

#[test]
fn binding_records_the_process_origin() {
    let identity = Identity::new();
    let session = identity.open(PeerCredentials::unauthenticated());
    let process = ProcessId::from_raw(7);
    identity.bind_process(session, process);
    assert_eq!(identity.origin(session), Origin::Process(process));
    assert_eq!(identity.origin(session).process(), Some(process));
}

#[test]
fn registering_records_an_external_label() {
    let identity = Identity::new();
    let session = identity.open(PeerCredentials::unauthenticated());
    identity.register_external(session, "claude-code".to_string());
    assert_eq!(
        identity.origin(session),
        Origin::External("claude-code".to_string())
    );
    // An external caller is bound to no supervised process.
    assert_eq!(identity.origin(session).process(), None);
}

#[test]
fn selecting_records_the_project() {
    let identity = Identity::new();
    let session = identity.open(PeerCredentials::unauthenticated());
    let project = ProjectId::from_raw(3);
    identity.select_project(session, project);
    assert_eq!(identity.selected_project(session), Some(project));
}

#[test]
fn selecting_records_the_process() {
    let identity = Identity::new();
    let session = identity.open(PeerCredentials::unauthenticated());
    let process = ProcessId::from_raw(5);
    identity.select_process(session, process);
    assert_eq!(identity.selected_process(session), Some(process));
}

#[test]
fn the_latest_origin_wins() {
    // A session that binds to a process and later registers a label keeps the label —
    // each call replaces the origin rather than accumulating.
    let identity = Identity::new();
    let session = identity.open(PeerCredentials::unauthenticated());
    identity.bind_process(session, ProcessId::from_raw(1));
    identity.register_external(session, "external".to_string());
    assert_eq!(
        identity.origin(session),
        Origin::External("external".to_string())
    );
}

#[test]
fn closing_drops_session_state() {
    let identity = Identity::new();
    let session = identity.open(PeerCredentials::unauthenticated());
    identity.bind_process(session, ProcessId::from_raw(1));
    identity.select_project(session, ProjectId::from_raw(1));
    identity.close(session);
    assert_eq!(identity.origin(session), Origin::Unbound);
    assert_eq!(identity.selected_project(session), None);
}

#[test]
fn an_unknown_session_reads_as_unbound() {
    let identity = Identity::new();
    let phantom = SessionId::from_raw(999);
    assert_eq!(identity.origin(phantom), Origin::Unbound);
    assert_eq!(identity.selected_project(phantom), None);
    assert_eq!(identity.peer_pgid(phantom), None);
}

#[test]
fn open_records_the_transport_peer_group_and_working_directory() {
    // The connecting peer's process group and working directory are recorded at open so the façade
    // can match a bind/select against them; a session opened without either (a transport that
    // cannot authenticate) reads as no peer group and no peer directory.
    let identity = Identity::new();

    let by_group = identity.open(PeerCredentials::in_group(4242));
    assert_eq!(identity.peer_pgid(by_group), Some(4242));
    assert_eq!(identity.peer_cwd(by_group), None);

    let by_dir = identity.open(PeerCredentials::in_dir(PathBuf::from(
        "/projects/storefront",
    )));
    assert_eq!(identity.peer_pgid(by_dir), None);
    assert_eq!(
        identity.peer_cwd(by_dir),
        Some(PathBuf::from("/projects/storefront"))
    );

    let unauthenticated = identity.open(PeerCredentials::unauthenticated());
    assert_eq!(identity.peer_pgid(unauthenticated), None);
    assert_eq!(identity.peer_cwd(unauthenticated), None);
}
