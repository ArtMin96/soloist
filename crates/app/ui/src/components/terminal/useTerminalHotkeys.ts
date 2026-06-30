import { useEffect, useRef, type RefObject } from "react";
import { FONT_SCALE_ORDER } from "@/lib/appearance";
import { bindingFromEvent, bindingsEqual } from "@/lib/hotkeys";
import { useAppearance } from "@/store/appearanceContext";
import { useHotkeys } from "@/store/hotkeysContext";
import type { ProcessView } from "@/domain";

// Intercepts terminal-scope hotkey chords in the capture phase so they are handled before
// xterm.js processes them (a capture listener fires before the target's own listeners, so
// the keystroke is never forwarded to the PTY). Installed once per mount via the passed ref.
export function useTerminalHotkeys(
  containerRef: RefObject<HTMLElement | null>,
  processes: ProcessView[],
  processId: number,
  onSelectProcess: ((id: number) => void) | undefined,
  onOpenSearch: (() => void) | undefined,
): void {
  const { bindings } = useHotkeys();
  const { appearance, setAppearance } = useAppearance();

  const bindingsRef = useRef(bindings);
  bindingsRef.current = bindings;

  const ctx = useRef({
    appearance,
    setAppearance,
    processes,
    processId,
    onSelectProcess,
    onOpenSearch,
  });
  ctx.current = { appearance, setAppearance, processes, processId, onSelectProcess, onOpenSearch };

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    function handleKey(event: KeyboardEvent) {
      const pressed = bindingFromEvent(event);
      if (!pressed) return;

      for (const row of bindingsRef.current) {
        if (row.scope !== "terminal" || !row.binding || !bindingsEqual(row.binding, pressed))
          continue;

        const {
          appearance: ap,
          setAppearance: setAp,
          processes: ps,
          processId: pid,
          onSelectProcess: onSel,
          onOpenSearch: onSearch,
        } = ctx.current;

        switch (row.action) {
          case "increase_terminal_font_size": {
            const idx = FONT_SCALE_ORDER.indexOf(ap.terminal.font_scale);
            if (idx < FONT_SCALE_ORDER.length - 1)
              setAp({ ...ap, terminal: { ...ap.terminal, font_scale: FONT_SCALE_ORDER[idx + 1] } });
            break;
          }
          case "decrease_terminal_font_size": {
            const idx = FONT_SCALE_ORDER.indexOf(ap.terminal.font_scale);
            if (idx > 0)
              setAp({ ...ap, terminal: { ...ap.terminal, font_scale: FONT_SCALE_ORDER[idx - 1] } });
            break;
          }
          case "open_terminal_search": {
            onSearch?.();
            break;
          }
          case "next_process": {
            const idx = ps.findIndex((p) => p.id === pid);
            if (idx !== -1 && idx < ps.length - 1) onSel?.(ps[idx + 1].id);
            break;
          }
          case "previous_process": {
            const idx = ps.findIndex((p) => p.id === pid);
            if (idx > 0) onSel?.(ps[idx - 1].id);
            break;
          }
          default:
            continue;
        }

        event.preventDefault();
        event.stopPropagation();
        return;
      }
    }

    el.addEventListener("keydown", handleKey, { capture: true });
    return () => el.removeEventListener("keydown", handleKey, { capture: true });
  }, []); // stable: refs supply latest values on every invocation
}
