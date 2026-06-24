//! The HTTP client half of the CLI: resolve the loopback base URL the running app recorded,
//! issue each command's request, and translate transport and status failures into the CLI's
//! typed errors. The local-auth header rides every mutation; reads are open on loopback.

use std::fmt;

use serde::de::DeserializeOwned;

use soloist_ipc::http::{
    read_runtime, DEFAULT_PORT, LOCAL_AUTH_HEADER, LOCAL_AUTH_VALUE, STATUS_FORBIDDEN,
    STATUS_NOT_FOUND, STATUS_UNAUTHORIZED,
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

/// A handle to the loopback API at a resolved base URL.
pub struct Client {
    base: String,
}

impl Client {
    /// Resolves the base URL from the port the running server recorded, falling back to the
    /// default loopback port when the runtime file is absent. The server rewrites the file on
    /// every bind, so a present file always names the live port; an absent one means "try the
    /// default — the app may simply not be running", which a refused connection then reports.
    pub fn from_runtime() -> Self {
        let port = read_runtime().map_or(DEFAULT_PORT, |runtime| runtime.port);
        Self::at_port(port)
    }

    /// The client for the loopback API on `port`.
    pub fn at_port(port: u16) -> Self {
        Self {
            base: format!("http://127.0.0.1:{port}"),
        }
    }

    /// `GET path` decoded as JSON `T`. A refused connection reads as [`CliError::NotRunning`];
    /// an error status or unreadable/unparseable body is surfaced as itself.
    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, CliError> {
        let url = self.url(path);
        let body = ureq::get(&url)
            .call()
            .map_err(read_error)?
            .body_mut()
            .read_to_string()
            .map_err(|err| CliError::Protocol(format!("could not read the response: {err}")))?;
        serde_json::from_str(&body)
            .map_err(|err| CliError::Protocol(format!("unexpected response from the API: {err}")))
    }

    /// `POST path` carrying the local-auth header and an empty body — every mutation. The
    /// status the adapter maps from a core outcome becomes a clear message; a refused
    /// connection reads as [`CliError::NotRunning`].
    pub fn post(&self, path: &str) -> Result<(), CliError> {
        let url = self.url(path);
        ureq::post(&url)
            .header(LOCAL_AUTH_HEADER, LOCAL_AUTH_VALUE)
            .send_empty()
            .map(|_| ())
            .map_err(mutation_error)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
}

/// Maps a read failure: an error status is reported with its code, anything else (a refused or
/// dropped connection on loopback) means the app is not there to answer.
fn read_error(err: ureq::Error) -> CliError {
    match err {
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
            CliError::Request("no such process or project".to_string())
        }
        ureq::Error::StatusCode(STATUS_UNAUTHORIZED) => {
            CliError::Request("the local-auth header was rejected".to_string())
        }
        ureq::Error::StatusCode(code) => CliError::Request(format!("the API returned HTTP {code}")),
        _ => CliError::NotRunning,
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
