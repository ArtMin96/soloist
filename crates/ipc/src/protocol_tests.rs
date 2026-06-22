use super::*;
use soloist_core::{ProcessId, ProjectId, ProjectView};
use std::path::PathBuf;

#[test]
fn requests_round_trip_through_json() {
    let requests = [
        IpcRequest::Whoami,
        IpcRequest::BindSessionProcess {
            process: ProcessId::from_raw(1),
        },
        IpcRequest::RegisterAgent {
            label: "claude-code".into(),
        },
        IpcRequest::SelectProject {
            project: ProjectId::from_raw(2),
        },
        IpcRequest::ListProjects,
        IpcRequest::GetProjectStatus {
            project: Some(ProjectId::from_raw(3)),
        },
        IpcRequest::GetProjectStatus { project: None },
        IpcRequest::ListProcesses,
        IpcRequest::GetProcessStatus {
            process: ProcessId::from_raw(4),
        },
    ];
    for request in requests {
        let json = serde_json::to_string(&request).expect("serialize");
        let back: IpcRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, request);
    }
}

#[test]
fn a_typed_error_round_trips() {
    for error in [
        IpcError::UnknownProcess,
        IpcError::UnknownProject,
        IpcError::NoProjectScope,
        IpcError::Internal("disk full".into()),
    ] {
        let json = serde_json::to_string(&error).expect("serialize");
        let back: IpcError = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, error);
    }
}

#[test]
fn a_project_summary_drops_the_ui_icon() {
    // The agent-facing projection keeps identity and root but never the icon data-URL.
    let view = ProjectView {
        id: ProjectId::from_raw(5),
        name: "storefront".into(),
        root: PathBuf::from("/projects/storefront"),
        icon: Some("data:image/png;base64,AAAA".into()),
    };
    let summary = ProjectSummary::from_view(&view);
    assert_eq!(summary.id, view.id);
    assert_eq!(summary.name, view.name);
    assert_eq!(summary.root, view.root);

    // The serialized shape carries no icon field at all.
    let json = serde_json::to_string(&summary).expect("serialize");
    assert!(!json.contains("icon"), "summary must not ship the UI icon");
}
