use super::*;

use std::os::unix::fs::PermissionsExt;

fn make_executable(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).expect("write binary");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("mark executable");
}

#[test]
fn helper_command_prefers_the_exported_data_dir_copy() {
    let install = tempfile::tempdir().expect("install dir");
    let data = tempfile::tempdir().expect("data dir");
    make_executable(&install.path().join(MCP_HELPER_BIN), b"");
    let exported = companion_bins::exported_path(data.path(), MCP_HELPER_BIN);
    std::fs::create_dir_all(exported.parent().expect("bin dir")).expect("create bin dir");
    make_executable(&exported, b"");

    assert_eq!(
        helper_command(Some(install.path().join("soloist")), data.path()),
        exported.display().to_string(),
        "the stable exported copy outranks the sibling — it survives an AppImage remount"
    );
}

#[test]
fn helper_command_falls_back_to_an_executable_sibling_binary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("data dir");
    let exe = dir.path().join("soloist");
    let sibling = dir.path().join(MCP_HELPER_BIN);
    make_executable(&sibling, b"");

    assert_eq!(
        helper_command(Some(exe), data.path()),
        sibling.display().to_string(),
        "with no exported copy, an installed helper next to the app binary is referenced absolutely"
    );
}

#[test]
fn helper_command_ignores_a_non_executable_sibling() {
    let dir = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("data dir");
    let sibling = dir.path().join(MCP_HELPER_BIN);
    std::fs::write(&sibling, b"").expect("create a same-named data file");
    std::fs::set_permissions(&sibling, std::fs::Permissions::from_mode(0o644))
        .expect("strip the execute bits");

    assert_eq!(
        helper_command(Some(dir.path().join("soloist")), data.path()),
        MCP_HELPER_BIN,
        "a name collision that cannot run must not be handed to a client"
    );
}

#[test]
fn helper_command_falls_back_to_the_bare_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let data = tempfile::tempdir().expect("data dir");

    // No exported copy, no sibling helper on disk, and no exe path at all: every layer
    // falls through to PATH lookup.
    assert_eq!(
        helper_command(Some(dir.path().join("soloist")), data.path()),
        MCP_HELPER_BIN
    );
    assert_eq!(helper_command(None, data.path()), MCP_HELPER_BIN);
}
