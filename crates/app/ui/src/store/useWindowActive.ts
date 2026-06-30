import { useEffect } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { isWindowFocused, onWindowFocusChanged } from "@/lib/window";

// Reflects whether Soloist's window is the key (focused) window onto the document root as
// `data-window-active`, so the AppKit "unemphasized" selection (a neutral tint on a background
// window, see index.css) stays a pure CSS concern. Defaults to active; only flips to "false"
// on blur. A no-op outside a Tauri window (a plain browser / test host).
export function useWindowActive(): void {
  useEffect(() => {
    let active = true;
    let unlisten: Promise<UnlistenFn> | null = null;
    const apply = (focused: boolean) => {
      if (active) document.documentElement.dataset.windowActive = String(focused);
    };
    try {
      void isWindowFocused()
        .then(apply)
        .catch(() => {});
      unlisten = onWindowFocusChanged(apply);
    } catch {
      // No Tauri window here; selection keeps its default active appearance.
    }
    return () => {
      active = false;
      void unlisten?.then((off) => off()).catch(() => {});
    };
  }, []);
}
