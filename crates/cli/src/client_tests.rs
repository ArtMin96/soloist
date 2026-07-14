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
fn carried_messages_render_verbatim() {
    let resolve = CliError::Resolve("no process named \"web\"".to_string());
    assert_eq!(resolve.to_string(), "no process named \"web\"");
    let request = CliError::Request("the API returned HTTP 500".to_string());
    assert_eq!(request.to_string(), "the API returned HTTP 500");
}
