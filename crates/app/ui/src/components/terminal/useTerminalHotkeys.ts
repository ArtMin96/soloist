import { useEffect, useRef, type RefObject } from "react";
import type { TerminalClipboard } from "@/components/terminal/terminalClipboard";
import { FONT_SCALE_ORDER } from "@/lib/appearance";
import { bindingFromEvent, matchHotkey } from "@/lib/hotkeys";
import { useAppearance, type AppearanceState } from "@/store/appearanceContext";
import { useHotkeys } from "@/store/hotkeysContext";
import type { ProcessView } from "@/domain";

/** What the terminal-scope actions need to act on, as the pane composing them supplies it. */
export interface TerminalHotkeysOptions {
  /** The element the capture-phase listener is installed on. */
  containerRef: RefObject<HTMLElement | null>;
  /** The panes the next/previous-process actions cycle through. */
  processes: ProcessView[];
  /** Which of `processes` this pane is showing, the cycle's starting point. */
  processId: number;
  onSelectProcess?: (id: number) => void;
  onOpenSearch?: () => void;
  clipboard: TerminalClipboard;
}

// Everything an action reads at the moment it fires. Held in a ref rather than closed over, so the
// listener can be installed once and still see the current render's values.
type LiveContext = TerminalHotkeysOptions & Pick<AppearanceState, "appearance" | "setAppearance">;

// Intercepts terminal-scope hotkey chords in the capture phase so they are handled before
// xterm.js processes them (a capture listener fires before the target's own listeners, so
// the keystroke is never forwarded to the PTY). Installed once per mount via the passed ref.
export function useTerminalHotkeys(options: TerminalHotkeysOptions): void {
  const { containerRef } = options;
  const { bindings } = useHotkeys();
  const { appearance, setAppearance } = useAppearance();

  const bindingsRef = useRef(bindings);
  bindingsRef.current = bindings;

  const ctx = useRef({} as LiveContext);
  ctx.current = { ...options, appearance, setAppearance };

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    function handleKey(event: KeyboardEvent) {
      const pressed = bindingFromEvent(event);
      if (!pressed) return;

      const action = matchHotkey(bindingsRef.current, "terminal", pressed);
      if (!action) return;

      const {
        appearance: ap,
        setAppearance: setAp,
        processes: ps,
        processId: pid,
        onSelectProcess: onSel,
        onOpenSearch: onSearch,
        clipboard: clip,
      } = ctx.current;

      switch (action) {
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
        // Both stay synchronous so the chord is swallowed before xterm can forward it to the PTY;
        // the clipboard work they start settles on its own and never throws back in here.
        case "copy_selection": {
          clip.copySelection();
          break;
        }
        case "paste_clipboard": {
          clip.paste();
          break;
        }
        default:
          return;
      }

      event.preventDefault();
      event.stopPropagation();
    }

    el.addEventListener("keydown", handleKey, { capture: true });
    return () => el.removeEventListener("keydown", handleKey, { capture: true });
  }, [containerRef]); // attach once (stable ref); live values are read through refs
}
