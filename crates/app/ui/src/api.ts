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
  McpFeatureGroup,
  McpToolGroups,
  ProcessView,
  ProjectLoad,
  ProjectView,
  Sidebar,
  ToolDefaults,
} from "@/domain";

const DOMAIN_EVENT = "domain-event";

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function procList(): Promise<ProcessView[]> {
  return invoke<ProcessView[]>("proc_list");
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

// Attaches the terminal pane to a process. The first channel message is the raw
// scrollback replay; subsequent messages are live PTY bytes. Returns the Channel so the
// caller can stop receiving (drop it) when the pane switches away or unmounts; pair with
// `ptyDetach` to cancel the backend forwarder.
export function ptyAttach(
  id: number,
  onChunk: (bytes: Uint8Array) => void,
): Promise<Channel<Uint8Array>> {
  const channel = new Channel<Uint8Array>();
  channel.onmessage = onChunk;
  return invoke<void>("pty_attach", { id, onChunk: channel }).then(() => channel);
}

export function ptyDetach(): Promise<void> {
  return invoke<void>("pty_detach");
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

export function onDomainEvent(handler: (event: DomainEvent) => void): Promise<UnlistenFn> {
  return listen<DomainEvent>(DOMAIN_EVENT, (event) => handler(event.payload));
}
