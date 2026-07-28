import { useEffect, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { isWindowFocused, onWindowFocusChanged } from "@/lib/window";

// Whether Soloist's window is the key (focused) window, or null until the first read answers. The
// one place focus is observed, so the surfaces that care — the unemphasized-selection styling and
// the presence Soloist reports to the core — cannot disagree about it.
//
// The unknown state is distinct on purpose: it lasts one paint, and the two consumers want
// opposite things from it. Styling must not flash the window as inactive before the answer
// arrives, while presence must not claim the user is here before anyone has looked. Outside a
// Tauri window (a plain browser / test host) it stays unknown.
export function useWindowFocused(): boolean | null {
  const [focused, setFocused] = useState<boolean | null>(null);

  useEffect(() => {
    let live = true;
    let unlisten: Promise<UnlistenFn> | null = null;
    const apply = (next: boolean) => {
      if (live) setFocused(next);
    };
    try {
      void isWindowFocused()
        .then(apply)
        .catch(() => {});
      unlisten = onWindowFocusChanged(apply);
    } catch {
      // No Tauri window here.
    }
    return () => {
      live = false;
      void unlisten?.then((off) => off()).catch(() => {});
    };
  }, []);

  return focused;
}
