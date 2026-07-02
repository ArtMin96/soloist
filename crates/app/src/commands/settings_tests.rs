use super::*;

use std::os::unix::fs::PermissionsExt;

#[test]
fn helper_command_prefers_an_executable_sibling_binary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let exe = dir.path().join("soloist");
    let sibling = dir.path().join(MCP_HELPER_BIN);
    std::fs::write(&sibling, b"").expect("create a sibling helper");
    std::fs::set_permissions(&sibling, std::fs::Permissions::from_mode(0o755))
        .expect("mark the helper executable");

    assert_eq!(
        helper_command(Some(exe)),
        sibling.display().to_string(),
        "an installed helper next to the app binary is referenced absolutely"
    );
}

#[test]
fn helper_command_ignores_a_non_executable_sibling() {
    let dir = tempfile::tempdir().expect("temp dir");
    let sibling = dir.path().join(MCP_HELPER_BIN);
    std::fs::write(&sibling, b"").expect("create a same-named data file");
    std::fs::set_permissions(&sibling, std::fs::Permissions::from_mode(0o644))
        .expect("strip the execute bits");

    assert_eq!(
        helper_command(Some(dir.path().join("soloist"))),
        MCP_HELPER_BIN,
        "a name collision that cannot run must not be handed to a client"
    );
}

#[test]
fn helper_command_falls_back_to_the_bare_name() {
    let dir = tempfile::tempdir().expect("temp dir");

    // No sibling helper on disk, and no exe path at all: both fall back to PATH lookup.
    assert_eq!(
        helper_command(Some(dir.path().join("soloist"))),
        MCP_HELPER_BIN
    );
    assert_eq!(helper_command(None), MCP_HELPER_BIN);
}
