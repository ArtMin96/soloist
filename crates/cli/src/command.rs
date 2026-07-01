//! The CLI's command handlers and the pure resolution and formatting they share. Each handler
//! is thin: resolve any name to the id the API is keyed by, issue **one** request to the same
//! core method the UI and MCP drive, and render the result. The only CLI-side logic is
//! name→id resolution and table rendering — and both are pure functions, unit-tested below.

use std::fmt::Write as _;

use soloist_ipc::http::{SpawnRequest, SpawnResponse};
use soloist_ipc::{ProcStatus, ProcessKind, ProcessView, ProjectView};

use crate::cli::{StatusFilter, Target};
use crate::client::{CliError, Client};

/// The literal a `start`/`stop`/`restart` target uses to mean "the whole project".
const ALL: &str = "all";

/// A process-control verb and the endpoints it maps to: the per-process action and the
/// project-wide bulk action. The bulk scopes mirror the HTTP and MCP semantics:
/// `start-all`/`restart-all` act on the trusted commands, `stop-all` on every live process.
#[derive(Debug, Clone, Copy)]
pub enum Verb {
    Start,
    Stop,
    Restart,
}

impl Verb {
    /// The `POST /processes/:id/<action>` path segment.
    fn process_action(self) -> &'static str {
        match self {
            Verb::Start => "start",
            Verb::Stop => "stop",
            Verb::Restart => "restart",
        }
    }

    /// The `POST /projects/:id/<action>` bulk path segment.
    fn bulk_action(self) -> &'static str {
        match self {
            Verb::Start => "start-all",
            Verb::Stop => "stop-all",
            Verb::Restart => "restart-all",
        }
    }

    /// The past-tense confirmation for one process.
    fn past(self) -> &'static str {
        match self {
            Verb::Start => "Started",
            Verb::Stop => "Stopped",
            Verb::Restart => "Restarted",
        }
    }

    /// The confirmation for a project-wide action, naming the verb's actual scope.
    fn bulk_summary(self) -> &'static str {
        match self {
            Verb::Start => "Started all commands in",
            Verb::Stop => "Stopped all processes in",
            Verb::Restart => "Restarted all commands in",
        }
    }
}

/// Runs `status`: fetch the process read model, optionally filter by status, render a table.
pub fn status(client: &Client, filter: Option<StatusFilter>) -> Result<String, CliError> {
    let processes: Vec<ProcessView> = client.get_json("/processes")?;
    Ok(render_table(&processes, filter))
}

/// Runs a control verb against one named process, or — when the target is `all` — against a
/// whole project's bulk endpoint. Routes to the same core command the UI button does.
pub fn control(client: &Client, verb: Verb, target: &Target) -> Result<String, CliError> {
    if target.target.eq_ignore_ascii_case(ALL) {
        let project = resolve_project(client, target.project.as_deref())?;
        client.post(&format!("/projects/{}/{}", project.id, verb.bulk_action()))?;
        Ok(format!(
            "{} project {:?}.",
            verb.bulk_summary(),
            project.name
        ))
    } else {
        let processes: Vec<ProcessView> = client.get_json("/processes")?;
        let process = resolve_process(&processes, &target.target)?;
        client.post(&format!(
            "/processes/{}/{}",
            process.id,
            verb.process_action()
        ))?;
        Ok(format!("{} {:?}.", verb.past(), process.label))
    }
}

/// Runs `logs`: resolve the process, then print its recent output from the read endpoint.
pub fn logs(client: &Client, name: &str, lines: Option<usize>) -> Result<String, CliError> {
    let processes: Vec<ProcessView> = client.get_json("/processes")?;
    let process = resolve_process(&processes, name)?;
    let path = match lines {
        Some(n) => format!("/processes/{}/output?lines={n}", process.id),
        None => format!("/processes/{}/output", process.id),
    };
    let output: Vec<String> = client.get_json(&path)?;
    Ok(output.join("\n"))
}

/// Raises the desktop window — the shared handler behind both `focus` and `open` (Solo's
/// `open` raise-app case is the same action, so it routes to the one `POST /focus` endpoint
/// rather than a second core path).
pub fn raise(client: &Client) -> Result<String, CliError> {
    client.post("/focus")?;
    Ok("Raised the Soloist window.".to_string())
}

/// Runs `spawn`: launch a configured agent tool as a worker in a project and start it. Resolves
/// the target project the same way a bulk `all` target does (the sole open project, or
/// `--project <name>`), then posts to the one core launch path — the same `launch_agent` the
/// desktop launch picker drives — and reports the new process.
pub fn spawn(
    client: &Client,
    tool: &str,
    args: &[String],
    project: Option<&str>,
) -> Result<String, CliError> {
    let project = resolve_project(client, project)?;
    let request = SpawnRequest {
        tool: tool.to_string(),
        args: args.to_vec(),
    };
    let spawned: SpawnResponse =
        client.post_json(&format!("/projects/{}/spawn-agent", project.id), &request)?;
    Ok(format!(
        "Spawned {tool:?} in project {:?} (process {}).",
        project.name, spawned.id
    ))
}

/// Resolves a process name to its row against the live stack, or a clear error when no process
/// — or more than one — bears that name. The API is keyed by id; a name is a CLI convenience
/// and a `label` is not guaranteed unique across projects, so an ambiguous name is refused
/// rather than guessed (mirroring the core's never-guess scope discipline).
fn resolve_process<'a>(
    processes: &'a [ProcessView],
    name: &str,
) -> Result<&'a ProcessView, CliError> {
    let matches: Vec<&ProcessView> = processes.iter().filter(|p| p.label == name).collect();
    match matches.as_slice() {
        [] => Err(CliError::Resolve(format!("no process named {name:?}"))),
        [only] => Ok(only),
        many => {
            let ids = many
                .iter()
                .map(|p| p.id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(CliError::Resolve(format!(
                "{} processes are named {name:?} (ids {ids}) — names are ambiguous across projects",
                many.len()
            )))
        }
    }
}

/// Fetches the open projects and picks the one to act on for a bulk command.
fn resolve_project(client: &Client, explicit: Option<&str>) -> Result<ProjectView, CliError> {
    let projects: Vec<ProjectView> = client.get_json("/projects")?;
    pick_project(projects, explicit)
}

/// Picks the project a bulk command targets: the named one if given, else the sole open
/// project, else an error — `all` is unambiguous only with one project or an explicit
/// `--project` (mirrors the MCP single-project default).
fn pick_project(
    projects: Vec<ProjectView>,
    explicit: Option<&str>,
) -> Result<ProjectView, CliError> {
    match explicit {
        Some(name) => projects
            .into_iter()
            .find(|project| project.name == name)
            .ok_or_else(|| CliError::Resolve(format!("no open project named {name:?}"))),
        None => {
            let mut projects = projects.into_iter();
            match (projects.next(), projects.next()) {
                (None, _) => Err(CliError::Resolve("no projects are open".to_string())),
                (Some(only), None) => Ok(only),
                (Some(_), Some(_)) => Err(CliError::Resolve(
                    "more than one project is open — pass --project <name>".to_string(),
                )),
            }
        }
    }
}

/// Renders the process read model as an aligned table, filtered to one status when asked.
fn render_table(processes: &[ProcessView], filter: Option<StatusFilter>) -> String {
    let rows: Vec<&ProcessView> = processes
        .iter()
        .filter(|p| filter.is_none_or(|f| status_matches(p.status, f)))
        .collect();
    if rows.is_empty() {
        return "No processes.".to_string();
    }
    let cells: Vec<[String; 5]> = rows
        .iter()
        .map(|p| {
            [
                p.id.to_string(),
                p.label.clone(),
                kind_label(p.kind).to_string(),
                status_label(p.status).to_string(),
                ports_label(&p.ports),
            ]
        })
        .collect();
    let headers = ["ID", "NAME", "KIND", "STATUS", "PORTS"];
    let widths = column_widths(&headers, &cells);
    let mut out = String::new();
    write_row(&mut out, &headers.map(String::from), &widths);
    for row in &cells {
        write_row(&mut out, row, &widths);
    }
    out.trim_end().to_string()
}

/// The widest cell in each column, header included, so columns line up. Measured in characters
/// (not bytes), matching how the formatter pads, so a multibyte label does not skew alignment.
fn column_widths(headers: &[&str; 5], rows: &[[String; 5]]) -> [usize; 5] {
    let mut widths = headers.map(|header| header.chars().count());
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    widths
}

/// Writes one left-aligned, padded row; the last column carries no trailing padding.
fn write_row(out: &mut String, cells: &[String; 5], widths: &[usize; 5]) {
    let last = cells.len() - 1;
    for (i, cell) in cells.iter().enumerate() {
        if i == last {
            let _ = write!(out, "{cell}");
        } else {
            let _ = write!(out, "{cell:<width$}  ", width = widths[i]);
        }
    }
    out.push('\n');
}

/// Whether a process's status matches the requested filter.
fn status_matches(status: ProcStatus, filter: StatusFilter) -> bool {
    matches!(
        (status, filter),
        (ProcStatus::Running, StatusFilter::Running) | (ProcStatus::Crashed, StatusFilter::Crashed)
    )
}

/// The lowercase status label shown in the table.
fn status_label(status: ProcStatus) -> &'static str {
    match status {
        ProcStatus::Stopped => "stopped",
        ProcStatus::Starting => "starting",
        ProcStatus::Running => "running",
        ProcStatus::Crashed => "crashed",
        ProcStatus::Restarting => "restarting",
        ProcStatus::Stopping => "stopping",
        ProcStatus::RestartExhausted => "exhausted",
    }
}

/// The lowercase kind label shown in the table.
fn kind_label(kind: ProcessKind) -> &'static str {
    match kind {
        ProcessKind::Command => "command",
        ProcessKind::Agent => "agent",
        ProcessKind::Terminal => "terminal",
    }
}

/// The ports cell: a comma-separated list, or `-` when none are bound.
fn ports_label(ports: &[u16]) -> String {
    if ports.is_empty() {
        "-".to_string()
    } else {
        ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
