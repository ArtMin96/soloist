import { useEffect } from "react";
import { bindingFromEvent, bindingsEqual } from "@/lib/hotkeys";
import { useHotkeys } from "@/store/hotkeysContext";
import type { HotkeyAction } from "@/domain";

// The live keyboard handler, driven by the remappable keymap (closes I6): a keydown is matched
// against the enabled General-scope bindings, and the matching action's handler runs if one is
// provided. General actions are app-wide; scoped (Sidebar/Terminal) dispatch lands with those
// features, so only General is matched globally today. Wiring a new action is one map entry.
// `handlers` must be stable (memoized by the caller) so the listener is not re-bound each render.
export function useGlobalHotkeys(handlers: Partial<Record<HotkeyAction, () => void>>) {
  const { bindings } = useHotkeys();

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const pressed = bindingFromEvent(event);
      if (!pressed) return;
      for (const row of bindings) {
        if (row.scope !== "general" || !row.binding) continue;
        if (bindingsEqual(row.binding, pressed)) {
          const handler = handlers[row.action];
          if (handler) {
            event.preventDefault();
            handler();
          }
          return;
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [bindings, handlers]);
}
