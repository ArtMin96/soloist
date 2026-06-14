// Typed wrapper over Tauri IPC: the UI reaches the core only through this module
// (invoke for commands, listen for events). No business logic lives in React.
import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  version: string;
}

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
