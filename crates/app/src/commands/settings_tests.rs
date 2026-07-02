use super::*;

#[test]
fn helper_command_prefers_an_existing_sibling_binary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let exe = dir.path().join("soloist");
    let sibling = dir.path().join(MCP_HELPER_BIN);
    std::fs::write(&sibling, b"").expect("create a sibling helper");

    assert_eq!(
        helper_command(Some(exe)),
        sibling.display().to_string(),
        "an installed helper next to the app binary is referenced absolutely"
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
