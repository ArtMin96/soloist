import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

// The single boundary to the OS window. Window-chrome actions (minimize, maximize,
// close) are a platform concern, not a domain command — so they bypass the Facade IPC
// in `api.ts` and talk to the Tauri window plugin directly through here.

export function minimizeWindow(): Promise<void> {
  return getCurrentWindow().minimize();
}

export function toggleMaximizeWindow(): Promise<void> {
  return getCurrentWindow().toggleMaximize();
}

export function closeWindow(): Promise<void> {
  return getCurrentWindow().close();
}

export function isWindowMaximized(): Promise<boolean> {
  return getCurrentWindow().isMaximized();
}

// Re-fires whenever the window is resized (maximize, restore, WM tiling, manual drag),
// so the UI can re-read the maximized state. Returns the unlisten handle for cleanup.
export function onWindowResized(handler: () => void): Promise<UnlistenFn> {
  return getCurrentWindow().onResized(() => handler());
}
