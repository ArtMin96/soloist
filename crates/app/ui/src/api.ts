// Typed wrapper over Tauri IPC: the UI reaches the core only through this module
// (invoke for commands, listen for events). Command/event names live here once; no
// business logic and no raw IPC strings leak into React.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppInfo, DomainEvent, ProcessView } from "@/domain";

const DOMAIN_EVENT = "domain-event";

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function listProcesses(): Promise<ProcessView[]> {
  return invoke<ProcessView[]>("list_processes");
}

export function spawnDemo(): Promise<number> {
  return invoke<number>("spawn_demo");
}

export function stopProcess(id: number): Promise<boolean> {
  return invoke<boolean>("stop_process", { id });
}

export function onDomainEvent(handler: (event: DomainEvent) => void): Promise<UnlistenFn> {
  return listen<DomainEvent>(DOMAIN_EVENT, (event) => handler(event.payload));
}
