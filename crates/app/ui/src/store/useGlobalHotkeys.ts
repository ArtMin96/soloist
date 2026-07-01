import { useEffect } from "react";
import { bindingFromEvent, hasCommandModifier, isEditableTarget, matchHotkey } from "@/lib/hotkeys";
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
      // A bare-key shortcut must not fire while the user is typing in a field; command-modifier
      // shortcuts (Ctrl/Alt/Super) still work everywhere, as in a native app.
      if (!hasCommandModifier(pressed) && isEditableTarget(event.target)) return;
      // Require a wired action so a matching but unwired chord can't swallow a chord a later,
      // handled action is bound to (a user-made conflict otherwise loses the working binding).
      const action = matchHotkey(bindings, "general", pressed, (a) => handlers[a] != null);
      if (!action) return;
      event.preventDefault();
      handlers[action]?.();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [bindings, handlers]);
}
