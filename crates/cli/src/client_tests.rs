use super::*;
use soloist_ipc::http::HttpRuntime;

#[test]
fn from_runtime_refuses_when_no_server_recorded_a_file() {
    // With no runtime file the app is either down or running as another user (its 0600 file
    // unreadable to us). Blindly hitting the default port could address a foreign server, so
    // the CLI refuses rather than guess — the same "not running" the acceptance walk expects.
    assert!(matches!(
        Client::from_runtime_opt(None),
        Err(CliError::NotRunning)
    ));
}

#[test]
fn from_runtime_uses_the_recorded_port_and_token() {
    let client = Client::from_runtime_opt(Some(HttpRuntime {
        port: 40000,
        token: "secret".to_string(),
    }))
    .expect("a recorded runtime yields a client");
    assert_eq!(client.url("/health"), "http://127.0.0.1:40000/health");
    assert_eq!(client.token, "secret");
}

#[test]
fn at_builds_a_loopback_base_url() {
    let client = Client::at(24678, "token");
    assert_eq!(client.url("/health"), "http://127.0.0.1:24678/health");
    assert_eq!(
        client.url("/processes/3/restart"),
        "http://127.0.0.1:24678/processes/3/restart"
    );
}

#[test]
fn not_running_renders_the_acceptance_message() {
    // The acceptance criterion: a clear "Soloist is not running" when the app is down.
    assert_eq!(CliError::NotRunning.to_string(), "Soloist is not running");
}

#[test]
fn a_forbidden_mutation_reads_as_a_trust_prompt() {
    // The adapter's status codes carry meaning: 403 is the trust gate, 404 an unknown target. The
    // mutation mapper turns each into its actionable message, so `soloist restart` on an untrusted
    // command surfaces "not trusted" rather than a bare status code.
    assert_eq!(
        mutation_error(ureq::Error::StatusCode(STATUS_FORBIDDEN)).to_string(),
        "that command is not trusted — trust it in Soloist first"
    );
    assert_eq!(
        mutation_error(ureq::Error::StatusCode(STATUS_NOT_FOUND)).to_string(),
        "no such process, project, or agent tool"
    );
    // The read path deliberately does not map 403 to the trust message — only mutations do.
    assert_eq!(
        read_error(ureq::Error::StatusCode(STATUS_FORBIDDEN)).to_string(),
        format!("the API returned HTTP {STATUS_FORBIDDEN}")
    );
}
