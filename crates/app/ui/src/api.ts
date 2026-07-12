// Typed wrapper over Tauri IPC: the UI reaches the core only through this module
// (invoke for commands, listen for events, a Channel for the PTY byte stream).
// Command/event names live here once; no business logic and no raw IPC strings leak
// into React.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AgentSettings,
  AgentTool,
  Appearance,
  AppInfo,
  Binding,
  DetectedTool,
  DomainEvent,
  HotkeyAction,
  HotkeyBindingView,
  Integrations,
  LineageEdge,
  McpFeatureGroup,
  McpSetupInfo,
  McpToolGroups,
  OrchestrationSnapshot,
  ProcessSpec,
  ProcessView,
  ProjectId,
  ProjectLoad,
  ProjectSettings,
  ProjectSettingsPage,
  ProjectView,
  ScratchpadDoc,
  ScratchpadView,
  Sidebar,
  TodoDoc,
  TodoView,
  ToolDefaults,
  TrustReviewCommand,
} from "@/domain";

const DOMAIN_EVENT = "domain-event";

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function procList(): Promise<ProcessView[]> {
  return invoke<ProcessView[]>("proc_list");
}

// The orchestration read model for one project — its agent lineage tree plus the coordination
// state agents share. Seeds the orchestration tree; a coordination or process-lifecycle domain
// event prompts a re-read (snapshot-then-deltas).
export function orchestrationSnapshot(project: number): Promise<OrchestrationSnapshot> {
  return invoke<OrchestrationSnapshot>("orchestration_snapshot", { project });
}

// Every live spawn-lineage edge across all projects — the sidebar joins these onto its process
// list to nest workers under their leads, re-reading on process lifecycle events.
export function lineageEdges(): Promise<LineageEdge[]> {
  return invoke<LineageEdge[]>("lineage_edges");
}

// --- Coordination panels: the scratchpad panel and the to-do board read/write through these.
// Each routes to the project-scoped core method; writes emit the domain event the panel re-reads on.

// The full scratchpad to open and edit (its disciplined document, rendering, and revision).
export function scratchpadRead(project: number, name: string): Promise<ScratchpadView> {
  return invoke<ScratchpadView>("scratchpad_read", { project, name });
}

// Save the scratchpad, revision-guarded by `expectedRevision` (null to create). A stale write
// rejects with the conflict message for the panel to surface.
export function scratchpadWrite(
  project: number,
  name: string,
  doc: ScratchpadDoc,
  expectedRevision: number | null,
): Promise<ScratchpadView> {
  return invoke<ScratchpadView>("scratchpad_write", {
    project,
    name,
    doc,
    expectedRevision,
  });
}

// Create a todo from its disciplined document.
export function todoCreate(project: number, doc: TodoDoc): Promise<TodoView> {
  return invoke<TodoView>("todo_create", { project, doc });
}

// Replace a todo's document, revision-guarded by `expectedRevision`.
export function todoUpdate(
  project: number,
  id: number,
  doc: TodoDoc,
  expectedRevision: number,
): Promise<TodoView> {
  return invoke<TodoView>("todo_update", { project, id, doc, expectedRevision });
}

// Mark a todo done; rejects while a blocker is unmet (the board surfaces the gate).
export function todoComplete(project: number, id: number): Promise<TodoView> {
  return invoke<TodoView>("todo_complete", { project, id });
}

// Replace a todo's blockers.
export function todoSetBlockers(
  project: number,
  id: number,
  blockers: number[],
): Promise<TodoView> {
  return invoke<TodoView>("todo_set_blockers", { project, id, blockers });
}

// Add one blocker to a todo.
export function todoAddBlocker(project: number, id: number, blocker: number): Promise<TodoView> {
  return invoke<TodoView>("todo_add_blocker", { project, id, blocker });
}

// Remove one blocker from a todo.
export function todoRemoveBlocker(project: number, id: number, blocker: number): Promise<TodoView> {
  return invoke<TodoView>("todo_remove_blocker", { project, id, blocker });
}

// The solo:// link to a scratchpad / todo, for the "Copy link" affordance.
export function scratchpadLink(project: number, scratchpad: number): Promise<string> {
  return invoke<string>("scratchpad_link", { project, scratchpad });
}

export function todoLink(project: number, todo: number): Promise<string> {
  return invoke<string>("todo_link", { project, todo });
}

// ── Timer management ──────────────────────────────────────────────────────────
// Routes to the existing core timer_cancel/pause/resume_for façade methods. `owner` is the process
// id of the timer's owning agent; `timer` is the timer id. Returns whether one was affected.

/** Cancels a timer owned by `owner`. */
export function timerCancel(owner: number, timer: number): Promise<boolean> {
  return invoke<boolean>("timer_cancel", { owner, timer });
}

/** Pauses a timer owned by `owner` (freezes the remaining time). */
export function timerPause(owner: number, timer: number): Promise<boolean> {
  return invoke<boolean>("timer_pause", { owner, timer });
}

/** Resumes a paused timer owned by `owner` (re-arms with remaining time). */
export function timerResume(owner: number, timer: number): Promise<boolean> {
  return invoke<boolean>("timer_resume", { owner, timer });
}

// The project read model — every opened project's identity with its icon already rendered
// (a data: URL). Seeds the sidebar's project tree; a `ProjectOpened` event prompts a re-read.
export function projectList(): Promise<ProjectView[]> {
  return invoke<ProjectView[]>("project_list");
}

// Opens the native folder picker for a project root (the directory holding solo.yml).
// Resolves to the chosen path, or null if the user cancelled.
export function openProjectDirectory(): Promise<string | null> {
  return open({ directory: true, multiple: false, title: "Open project" });
}

// Loads the project rooted at `path`: the core registers its commands and starts the
// trusted auto-start subset, emitting the events that repopulate the read model. Resolves
// to the new project's id and how many processes it declared (zero ⇒ no solo.yml found).
export function projectLoad(path: string): Promise<ProjectLoad> {
  return invoke<ProjectLoad>("project_load", { path });
}

// Removes a project from Soloist: the core closes its processes, deletes its durable
// state (trust, todos, scratchpads, settings, …), and emits `ProjectRemoved`, which
// prompts the project snapshot re-read. Files on disk are untouched.
export function projectRemove(project: number): Promise<void> {
  return invoke<void>("project_remove", { project });
}

// Trusts a project's command by name (the core trust gate) so it can start. The read
// model clears the command's blocked state; callers re-read the snapshot to reflect it.
export function configTrust(project: number, name: string): Promise<void> {
  return invoke<void>("config_trust", { project, name });
}

// Every configured agent tool, for the launch picker to render instantly (no probing).
export function agentList(): Promise<AgentTool[]> {
  return invoke<AgentTool[]>("agent_list");
}

// Each configured tool paired with whether its CLI appears installed (probes `--version`).
// Slower than `agentList`, so the picker lists first and fills in detection when this resolves.
export function agentDetect(): Promise<DetectedTool[]> {
  return invoke<DetectedTool[]>("agent_detect");
}

// Launches an agent tool as an interactive Agent process in `project` and starts it,
// resolving to its process id. `extraArgs` are appended for this one launch ("agent with
// flags"); pass [] for a plain launch.
export function agentLaunch(project: number, tool: string, extraArgs: string[]): Promise<number> {
  return invoke<number>("agent_launch", { project, tool, extraArgs });
}

export function procStart(id: number): Promise<void> {
  return invoke<void>("proc_start", { id });
}

export function procStop(id: number): Promise<boolean> {
  return invoke<boolean>("proc_stop", { id });
}

export function procRestart(id: number): Promise<void> {
  return invoke<void>("proc_restart", { id });
}

// Resumes a stopped agent's last session: relaunches it with its provider's resume command
// instead of starting fresh. Only meaningful for a resumable agent (ProcessView.resumable).
export function agentResume(id: number): Promise<void> {
  return invoke<void>("agent_resume", { id });
}

export function stackStart(project: number): Promise<void> {
  return invoke<void>("stack_start", { project });
}

export function stackStop(project: number): Promise<void> {
  return invoke<void>("stack_stop", { project });
}

export function stackRestartRunning(project: number): Promise<void> {
  return invoke<void>("stack_restart_running", { project });
}

export function ptyWrite(id: number, data: string): Promise<void> {
  return invoke<void>("pty_write", { id, data });
}

export function ptyResize(id: number, cols: number, rows: number): Promise<void> {
  return invoke<void>("pty_resize", { id, cols, rows });
}

// Byte 0 of every PTY frame tags the payload: a resync (a scrollback snapshot the emulator
// must reset to — sent first and again whenever the forwarder falls behind) versus a live
// chunk to append (tag 0). Mirrors the backend `PTY_FRAME_RESYNC` in `commands/mod.rs`.
const PTY_FRAME_RESYNC = 1;

// Attaches the terminal pane to a process. The first channel message is a resync carrying the
// raw scrollback replay; subsequent messages are live PTY bytes, with an occasional resync if
// the forwarder had to recover from falling behind. Each frame's first byte is the tag; the
// callback receives the payload (tag stripped) plus whether it is a resync. Resolves to the
// token that identifies this attachment: pass it to `ptyDetach` to cancel the backend forwarder.
export function ptyAttach(
  id: number,
  onChunk: (bytes: Uint8Array, resync: boolean) => void,
): Promise<number> {
  const channel = new Channel<Uint8Array>();
  channel.onmessage = (frame) => onChunk(frame.subarray(1), frame[0] === PTY_FRAME_RESYNC);
  return invoke<number>("pty_attach", { id, onChunk: channel });
}

// Detaches the attachment identified by `token`. Commands execute out of invoke order, so
// the backend ignores a stale token — a late detach never cancels a newer attachment.
export function ptyDetach(token: number): Promise<void> {
  return invoke<void>("pty_detach", { token });
}

// Resolves surfaced orphans: the pgids to SIGKILL and forget. An empty list ("Leave
// running") signals nothing.
export function orphansResolve(pgids: number[]): Promise<void> {
  return invoke<void>("orphans_resolve", { pgids });
}

// ── Settings ────────────────────────────────────────────────────────────────
// The durable global preference document. Reads return the stored value (or the documented
// defaults when nothing is stored yet); each setter auto-saves the whole tab and returns the
// stored value, so callers reflect exactly what was written without a re-read.

export function appearance(): Promise<Appearance> {
  return invoke<Appearance>("appearance");
}

export function setAppearance(appearance: Appearance): Promise<Appearance> {
  return invoke<Appearance>("set_appearance", { appearance });
}

export function sidebarSettings(): Promise<Sidebar> {
  return invoke<Sidebar>("sidebar_settings");
}

export function setSidebarSettings(sidebar: Sidebar): Promise<Sidebar> {
  return invoke<Sidebar>("set_sidebar_settings", { sidebar });
}

export function hotkeys(): Promise<HotkeyBindingView[]> {
  return invoke<HotkeyBindingView[]>("hotkeys");
}

export function remapHotkey(action: HotkeyAction, binding: Binding): Promise<HotkeyBindingView[]> {
  return invoke<HotkeyBindingView[]>("remap_hotkey", { action, binding });
}

export function disableHotkey(action: HotkeyAction): Promise<HotkeyBindingView[]> {
  return invoke<HotkeyBindingView[]>("disable_hotkey", { action });
}

export function resetHotkey(action: HotkeyAction): Promise<HotkeyBindingView[]> {
  return invoke<HotkeyBindingView[]>("reset_hotkey", { action });
}

export function resetAllHotkeys(): Promise<HotkeyBindingView[]> {
  return invoke<HotkeyBindingView[]>("reset_all_hotkeys");
}

export function agentSettings(): Promise<AgentSettings> {
  return invoke<AgentSettings>("agent_settings");
}

export function setAgentSettings(agents: AgentSettings): Promise<AgentSettings> {
  return invoke<AgentSettings>("set_agent_settings", { agents });
}

export function toolDefaults(): Promise<ToolDefaults> {
  return invoke<ToolDefaults>("tool_defaults");
}

export function setToolDefaults(tools: ToolDefaults): Promise<ToolDefaults> {
  return invoke<ToolDefaults>("set_tool_defaults", { tools });
}

export function integrationSettings(): Promise<Integrations> {
  return invoke<Integrations>("integration_settings");
}

export function setIntegrationSettings(integrations: Integrations): Promise<Integrations> {
  return invoke<Integrations>("set_integration_settings", { integrations });
}

export function mcpToolGroups(): Promise<McpToolGroups> {
  return invoke<McpToolGroups>("mcp_tool_groups");
}

export function setMcpToolGroup(group: McpFeatureGroup, enabled: boolean): Promise<McpToolGroups> {
  return invoke<McpToolGroups>("set_mcp_tool_group", { group, enabled });
}

export function mcpSetupInfo(): Promise<McpSetupInfo> {
  return invoke<McpSetupInfo>("mcp_setup_info");
}

// ── Per-project settings ──────────────────────────────────────────────────────
// The durable, app-local preferences for one project. The page query assembles the whole settings
// view in one call; each setter auto-saves and returns the stored settings; shared-command edits
// return the commands the `solo.yml` write left needing trust; the move transfers a command between
// the shared and local stores.

// The assembled settings page — root, config validity, command roster, and live counts.
export function projectSettingsPage(project: ProjectId): Promise<ProjectSettingsPage> {
  return invoke<ProjectSettingsPage>("project_settings_page", { project });
}

export function projectSettings(project: ProjectId): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("project_settings", { project });
}

export function setProjectAutoStartGate(
  project: ProjectId,
  engaged: boolean,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("set_project_auto_start_gate", { project, engaged });
}

export function setProjectAutoTrustCommandChanges(
  project: ProjectId,
  enabled: boolean,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("set_project_auto_trust_command_changes", { project, enabled });
}

export function setProjectEditorOverride(
  project: ProjectId,
  editor: string | null,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("set_project_editor_override", { project, editor });
}

export function setProjectCrashExitAlerts(
  project: ProjectId,
  enabled: boolean,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("set_project_crash_exit_alerts", { project, enabled });
}

export function setProjectTerminalAlerts(
  project: ProjectId,
  enabled: boolean,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("set_project_terminal_alerts", { project, enabled });
}

export function setCommandTerminalAlerts(
  project: ProjectId,
  command: string,
  enabled: boolean,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("set_command_terminal_alerts", { project, command, enabled });
}

export function addSharedCommand(
  project: ProjectId,
  name: string,
  spec: ProcessSpec,
): Promise<TrustReviewCommand[]> {
  return invoke<TrustReviewCommand[]>("add_shared_command", { project, name, spec });
}

export function editSharedCommand(
  project: ProjectId,
  name: string,
  spec: ProcessSpec,
): Promise<TrustReviewCommand[]> {
  return invoke<TrustReviewCommand[]>("edit_shared_command", { project, name, spec });
}

export function renameSharedCommand(
  project: ProjectId,
  from: string,
  to: string,
): Promise<TrustReviewCommand[]> {
  return invoke<TrustReviewCommand[]>("rename_shared_command", { project, from, to });
}

export function removeSharedCommand(
  project: ProjectId,
  name: string,
): Promise<TrustReviewCommand[]> {
  return invoke<TrustReviewCommand[]>("remove_shared_command", { project, name });
}

export function addLocalCommand(
  project: ProjectId,
  name: string,
  spec: ProcessSpec,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("add_local_command", { project, name, spec });
}

export function editLocalCommand(
  project: ProjectId,
  name: string,
  spec: ProcessSpec,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("edit_local_command", { project, name, spec });
}

export function renameLocalCommand(
  project: ProjectId,
  from: string,
  to: string,
): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("rename_local_command", { project, from, to });
}

export function removeLocalCommand(project: ProjectId, name: string): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("remove_local_command", { project, name });
}

export function makeCommandLocal(project: ProjectId, name: string): Promise<ProjectSettings> {
  return invoke<ProjectSettings>("make_command_local", { project, name });
}

export function saveCommandToYaml(project: ProjectId, name: string): Promise<TrustReviewCommand[]> {
  return invoke<TrustReviewCommand[]>("save_command_to_yaml", { project, name });
}

// Sets or clears (null) the project's solo.yml icon (shared). Rejects an .svg path server-side.
export function setProjectIcon(project: ProjectId, icon: string | null): Promise<void> {
  return invoke<void>("set_project_icon", { project, icon });
}

export function onDomainEvent(handler: (event: DomainEvent) => void): Promise<UnlistenFn> {
  return listen<DomainEvent>(DOMAIN_EVENT, (event) => handler(event.payload));
}

// The backend's delta stream fell behind and dropped events; stores must re-read their
// snapshots to recover, since a lost delta is otherwise permanent. Carries no payload.
const DOMAIN_RESYNC = "domain-resync";

export function onResync(handler: () => void): Promise<UnlistenFn> {
  return listen(DOMAIN_RESYNC, () => handler());
}
