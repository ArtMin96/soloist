use soloist_core::{ProcessId, ProjectId, Readiness};

use super::*;

/// A process row with the fields the CLI looks at; the rest are neutral defaults.
fn process(id: u64, project: u64, label: &str, status: ProcStatus, ports: Vec<u16>) -> ProcessView {
    ProcessView {
        id: ProcessId::from_raw(id),
        project: ProjectId::from_raw(project),
        kind: ProcessKind::Command,
        label: label.to_string(),
        status,
        exit_code: None,
        requires_trust: false,
        ports,
        ready: Readiness::Ungated,
    }
}

fn project(id: u64, name: &str) -> ProjectView {
    ProjectView {
        id: ProjectId::from_raw(id),
        name: name.to_string(),
        root: std::path::PathBuf::from("/"),
        icon: None,
    }
}

#[test]
fn resolve_process_finds_a_unique_name() {
    let procs = vec![
        process(1, 1, "web", ProcStatus::Running, vec![]),
        process(2, 1, "api", ProcStatus::Stopped, vec![]),
    ];
    assert_eq!(
        resolve_process(&procs, "api").expect("resolved").id.get(),
        2
    );
}

#[test]
fn resolve_process_rejects_an_unknown_name() {
    let procs = vec![process(1, 1, "web", ProcStatus::Running, vec![])];
    assert!(matches!(
        resolve_process(&procs, "nope"),
        Err(CliError::Resolve(_))
    ));
}

#[test]
fn resolve_process_refuses_an_ambiguous_name_rather_than_guessing() {
    // The same label in two projects must not be silently picked.
    let procs = vec![
        process(3, 1, "web", ProcStatus::Running, vec![]),
        process(7, 2, "web", ProcStatus::Stopped, vec![]),
    ];
    let CliError::Resolve(msg) = resolve_process(&procs, "web").unwrap_err() else {
        panic!("expected a resolve error");
    };
    assert!(
        msg.contains('3') && msg.contains('7'),
        "names both ids: {msg}"
    );
}

#[test]
fn pick_project_uses_the_sole_open_project() {
    let only = pick_project(vec![project(1, "storefront")], None).expect("picked");
    assert_eq!(only.name, "storefront");
}

#[test]
fn pick_project_requires_a_choice_when_several_are_open() {
    let err = pick_project(vec![project(1, "a"), project(2, "b")], None).unwrap_err();
    assert!(matches!(err, CliError::Resolve(_)));
}

#[test]
fn pick_project_honors_an_explicit_name() {
    let chosen = pick_project(vec![project(1, "a"), project(2, "b")], Some("b")).expect("picked");
    assert_eq!(chosen.id.get(), 2);
}

#[test]
fn pick_project_rejects_an_unknown_explicit_name() {
    let err = pick_project(vec![project(1, "a")], Some("zzz")).unwrap_err();
    assert!(matches!(err, CliError::Resolve(_)));
}

#[test]
fn pick_project_with_none_open_is_an_error() {
    assert!(matches!(
        pick_project(vec![], None),
        Err(CliError::Resolve(_))
    ));
}

#[test]
fn render_table_lists_rows_under_a_header() {
    let procs = vec![
        process(1, 1, "web", ProcStatus::Running, vec![3000]),
        process(2, 1, "api", ProcStatus::Crashed, vec![]),
    ];
    let table = render_table(&procs, None);
    for header in ["ID", "NAME", "KIND", "STATUS", "PORTS"] {
        assert!(table.contains(header), "header {header} present: {table}");
    }
    assert!(table.contains("web") && table.contains("running") && table.contains("3000"));
    assert!(table.contains("api") && table.contains("crashed"));
    // A process with no ports shows the placeholder, not an empty cell.
    assert!(table.contains('-'));
}

#[test]
fn render_table_filters_by_status() {
    let procs = vec![
        process(1, 1, "web", ProcStatus::Running, vec![]),
        process(2, 1, "api", ProcStatus::Crashed, vec![]),
    ];
    let running = render_table(&procs, Some(StatusFilter::Running));
    assert!(running.contains("web") && !running.contains("api"));
    let crashed = render_table(&procs, Some(StatusFilter::Crashed));
    assert!(crashed.contains("api") && !crashed.contains("web"));
}

#[test]
fn render_table_reports_an_empty_stack() {
    assert_eq!(render_table(&[], None), "No processes.");
    let only_running = vec![process(1, 1, "web", ProcStatus::Running, vec![])];
    assert_eq!(
        render_table(&only_running, Some(StatusFilter::Crashed)),
        "No processes."
    );
}
