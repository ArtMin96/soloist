//! The HTTP client half of the CLI: resolve the loopback base URL and per-launch token the
//! running app recorded, issue each command's request, and translate transport and status
//! failures into the CLI's typed errors. The token rides every request — reads and mutations
//! alike are gated.

use std::fmt;

use serde::de::DeserializeOwned;
use serde::Serialize;

use soloist_ipc::http::{
    read_runtime, DEFAULT_PORT, LOCAL_AUTH_HEADER, STATUS_FORBIDDEN, STATUS_NOT_FOUND,
    STATUS_UNAUTHORIZED,
};

/// Why a CLI command failed — each rendered to one stderr line by [`crate::run`].
#[derive(Debug, PartialEq, Eq)]
pub enum CliError {
    /// The loopback API could not be reached — almost always because Soloist is not running.
    NotRunning,
    /// A name or project could not be resolved to something to act on.
    Resolve(String),
    /// The server answered with an error status.
    Request(String),
    /// The response could not be read or parsed.
    Protocol(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::NotRunning => f.write_str("Soloist is not running"),
            CliError::Resolve(msg) | CliError::Request(msg) | CliError::Protocol(msg) => {
                f.write_str(msg)
            }
        }
    }
}

/// A handle to the loopback API at a resolved base URL, carrying the token to authenticate.
pub struct Client {
    base: String,
    token: String,
}

impl Client {
    /// Resolves the base URL and token from what the running server recorded, falling back to
    /// the default loopback port and an empty token when the runtime file is absent. The
    /// server rewrites the file on every bind, so a present file always names the live port
    /// and token; an absent one means "try the default — the app may simply not be running",
    /// which a refused connection then reports (or, if it is running without a readable file,
    /// a rejected empty token).
    pub fn from_runtime() -> Self {
        match read_runtime() {
            Some(runtime) => Self::at(runtime.port, runtime.token),
            None => Self::at(DEFAULT_PORT, String::new()),
        }
    }

    /// The client for the loopback API on `port` presenting `token`.
    pub fn at(port: u16, token: impl Into<String>) -> Self {
        Self {
            base: format!("http://127.0.0.1:{port}"),
            token: token.into(),
        }
    }

    /// `GET path` decoded as JSON `T`. A refused connection reads as [`CliError::NotRunning`];
    /// an error status or unreadable/unparseable body is surfaced as itself.
    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, CliError> {
        let url = self.url(path);
        let body = ureq::get(&url)
            .header(LOCAL_AUTH_HEADER, &self.token)
            .call()
            .map_err(read_error)?
            .body_mut()
            .read_to_string()
            .map_err(|err| CliError::Protocol(format!("could not read the response: {err}")))?;
        serde_json::from_str(&body)
            .map_err(|err| CliError::Protocol(format!("unexpected response from the API: {err}")))
    }

    /// `POST path` carrying the token and an empty body — every mutation. The status the
    /// adapter maps from a core outcome becomes a clear message; a refused connection reads as
    /// [`CliError::NotRunning`].
    pub fn post(&self, path: &str) -> Result<(), CliError> {
        let url = self.url(path);
        ureq::post(&url)
            .header(LOCAL_AUTH_HEADER, &self.token)
            .send_empty()
            .map(|_| ())
            .map_err(mutation_error)
    }

    /// `DELETE path` carrying the token — a resource-removing mutation. Status and transport
    /// failures map exactly as [`post`](Self::post) does.
    pub fn delete(&self, path: &str) -> Result<(), CliError> {
        let url = self.url(path);
        ureq::delete(&url)
            .header(LOCAL_AUTH_HEADER, &self.token)
            .call()
            .map(|_| ())
            .map_err(mutation_error)
    }

    /// `POST path` carrying the token and a JSON `body`, decoding the JSON response as `T` — a
    /// mutation that takes a request body and returns data (e.g. `spawn`). Status and transport
    /// failures map exactly as [`post`](Self::post) does.
    pub fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, CliError> {
        let url = self.url(path);
        let payload = serde_json::to_vec(body)
            .map_err(|err| CliError::Protocol(format!("could not encode the request: {err}")))?;
        let text = ureq::post(&url)
            .header(LOCAL_AUTH_HEADER, &self.token)
            .header("content-type", "application/json")
            .send(payload.as_slice())
            .map_err(mutation_error)?
            .body_mut()
            .read_to_string()
            .map_err(|err| CliError::Protocol(format!("could not read the response: {err}")))?;
        serde_json::from_str(&text)
            .map_err(|err| CliError::Protocol(format!("unexpected response from the API: {err}")))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
}

/// Maps a read failure: a rejected token is called out (the same actionable message the
/// mutations give), any other error status is reported with its code, and anything else (a
/// refused or dropped connection on loopback) means the app is not there to answer.
fn read_error(err: ureq::Error) -> CliError {
    match err {
        ureq::Error::StatusCode(STATUS_UNAUTHORIZED) => CliError::Request(token_rejected()),
        ureq::Error::StatusCode(code) => CliError::Request(format!("the API returned HTTP {code}")),
        _ => CliError::NotRunning,
    }
}

/// Maps a mutation failure: the adapter's status codes carry meaning (the trust gate is
/// `403`, an unknown target is `404`), so each becomes an actionable message; a transport
/// failure means the app is not running.
fn mutation_error(err: ureq::Error) -> CliError {
    match err {
        ureq::Error::StatusCode(STATUS_FORBIDDEN) => {
            CliError::Request("that command is not trusted — trust it in Soloist first".to_string())
        }
        ureq::Error::StatusCode(STATUS_NOT_FOUND) => {
            CliError::Request("no such process, project, or agent tool".to_string())
        }
        ureq::Error::StatusCode(STATUS_UNAUTHORIZED) => CliError::Request(token_rejected()),
        ureq::Error::StatusCode(code) => CliError::Request(format!("the API returned HTTP {code}")),
        _ => CliError::NotRunning,
    }
}

/// The message when the server rejects the token — most often because the file naming it was
/// unreadable (a different user than Soloist runs as), which is exactly the boundary the
/// token enforces.
fn token_rejected() -> String {
    "the auth token was rejected — run this as the same user Soloist is running as".to_string()
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
