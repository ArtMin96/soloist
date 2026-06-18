// Typed wrapper over Tauri IPC: the UI reaches the core only through this module
// (invoke for commands, listen for events, a Channel for the PTY byte stream).
// Command/event names live here once; no business logic and no raw IPC strings leak
// into React.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppInfo, DomainEvent, ProcessView } from "@/domain";

const DOMAIN_EVENT = "domain-event";

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function procList(): Promise<ProcessView[]> {
  return invoke<ProcessView[]>("proc_list");
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

export function onDomainEvent(handler: (event: DomainEvent) => void): Promise<UnlistenFn> {
  return listen<DomainEvent>(DOMAIN_EVENT, (event) => handler(event.payload));
}
