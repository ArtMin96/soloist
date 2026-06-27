import { useEffect } from "react";
import { bindingFromEvent, bindingsEqual, hasCommandModifier } from "@/lib/hotkeys";
import { useHotkeys } from "@/store/hotkeysContext";
import type { HotkeyAction } from "@/domain";

// True when the event originates in a text-editing surface (an input, textarea, or
// contenteditable — the terminal's helper textarea included), where a bare-key shortcut must
// yield to typing.
function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable;
}

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
      // A bare-key shortcut must not fire while the user is typing in a field; command-modifier
      // shortcuts (Ctrl/Alt/Super) still work everywhere, as in a native app.
      if (!hasCommandModifier(pressed) && isEditableTarget(event.target)) return;
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
