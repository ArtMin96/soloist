// Typed wrapper over Tauri IPC: the UI reaches the core only through this module
// (invoke for commands, listen for events, a Channel for the PTY byte stream).
// Command/event names live here once; no business logic and no raw IPC strings leak
// into React.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppInfo, DomainEvent, ProcessView, ProjectLoad, ProjectView } from "@/domain";

const DOMAIN_EVENT = "domain-event";

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function procList(): Promise<ProcessView[]> {
  return invoke<ProcessView[]>("proc_list");
}

// The project read model — every opened project's display identity. Seeds the sidebar's
// project tree; live opens arrive as `ProjectOpened` domain events.
export function projectList(): Promise<ProjectView[]> {
  return invoke<ProjectView[]>("project_list");
}

// Reads a project's icon (a resolved absolute path from `ProjectView.icon`) into a data:
// URL for an <img>, or null when absent/unreadable/too large/not an image.
export function projectIcon(path: string): Promise<string | null> {
  return invoke<string | null>("project_icon", { path });
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

export function onDomainEvent(handler: (event: DomainEvent) => void): Promise<UnlistenFn> {
  return listen<DomainEvent>(DOMAIN_EVENT, (event) => handler(event.payload));
}
