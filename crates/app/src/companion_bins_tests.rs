use super::*;

use std::os::unix::fs::PermissionsExt;

fn write_executable(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("write binary");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("mark executable");
}

#[test]
fn refresh_exports_an_executable_sibling_with_its_mode_bits() {
    let install = tempfile::tempdir().expect("install dir");
    let data = tempfile::tempdir().expect("data dir");
    write_executable(&install.path().join(MCP_HELPER_BIN), b"helper-v1");

    refresh(Some(install.path().join("soloist")), data.path());

    let exported = exported_path(data.path(), MCP_HELPER_BIN);
    assert_eq!(fs::read(&exported).expect("exported copy"), b"helper-v1");
    let mode = fs::metadata(&exported)
        .expect("exported metadata")
        .permissions()
        .mode();
    assert_ne!(mode & 0o111, 0, "the exported copy must stay executable");
}

#[test]
fn refresh_replaces_a_stale_export_and_keeps_a_fresh_one() {
    let install = tempfile::tempdir().expect("install dir");
    let data = tempfile::tempdir().expect("data dir");
    let sibling = install.path().join(CLI_BIN);
    write_executable(&sibling, b"cli-v2");
    let exported = exported_path(data.path(), CLI_BIN);
    fs::create_dir_all(exported.parent().expect("bin dir")).expect("create bin dir");
    write_executable(&exported, b"cli-v1");

    refresh(Some(install.path().join("soloist")), data.path());
    assert_eq!(
        fs::read(&exported).expect("refreshed copy"),
        b"cli-v2",
        "a stale export is replaced by the shipped binary"
    );

    assert!(
        !export(&sibling, &exported).expect("re-export"),
        "identical bytes are left untouched"
    );
}

#[test]
fn refresh_skips_a_non_executable_sibling_and_a_missing_exe() {
    let install = tempfile::tempdir().expect("install dir");
    let data = tempfile::tempdir().expect("data dir");
    let sibling = install.path().join(MCP_HELPER_BIN);
    fs::write(&sibling, b"data file").expect("write file");
    fs::set_permissions(&sibling, fs::Permissions::from_mode(0o644)).expect("strip exec");

    refresh(Some(install.path().join("soloist")), data.path());
    refresh(None, data.path());

    assert!(
        !exported_path(data.path(), MCP_HELPER_BIN).exists(),
        "a name collision that cannot run is never exported"
    );
}
