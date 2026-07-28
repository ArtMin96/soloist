import { useEffect } from "react";
import { useWindowFocused } from "@/store/useWindowFocused";

// Reflects whether Soloist's window is the key (focused) window onto the document root as
// `data-window-active`, so the AppKit "unemphasized" selection (a neutral tint on a background
// window, see index.css) stays a pure CSS concern. Absent means active, so nothing is written
// until focus is actually known — otherwise the window would flash unemphasized on every mount.
export function useWindowActive(): void {
  const focused = useWindowFocused();

  useEffect(() => {
    if (focused !== null) document.documentElement.dataset.windowActive = String(focused);
  }, [focused]);
}
