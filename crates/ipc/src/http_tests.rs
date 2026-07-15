//! The per-launch token and the owner-only runtime file: a token is high-entropy and fresh
//! each call (never the old constant), and the file the CLI reads round-trips port+token
//! and stays unreadable to other local users.

use super::*;

#[test]
fn a_token_is_hex_encoded_and_fresh_each_launch() {
    let first = generate_token().expect("token");
    let second = generate_token().expect("token");
    // 32 random bytes hex-encode to 64 printable characters.
    assert_eq!(first.len(), TOKEN_BYTES * 2);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    // A per-launch secret, not the old constant "1": two mints must differ.
    assert_ne!(first, second, "each launch mints a distinct token");
}

#[test]
fn the_runtime_file_round_trips_port_and_token_and_is_owner_only() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("http-api.json");
    let runtime = HttpRuntime {
        port: 24680,
        token: generate_token().expect("token"),
    };

    let json = serde_json::to_vec(&runtime).expect("serialize");
    write_owner_only(&path, &json).expect("write");

    // What the CLI reads back names the live port and the token to present.
    let read: HttpRuntime =
        serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("deserialize");
    assert_eq!(read, runtime);

    // The file carries the secret, so it is readable only by its owner.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            RUNTIME_FILE_MODE,
            "runtime file is owner-only"
        );
    }
}
