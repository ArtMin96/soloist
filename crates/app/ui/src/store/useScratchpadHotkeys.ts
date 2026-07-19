import { useEffect, type RefObject } from "react";
import { bindingFromEvent, matchHotkey } from "@/lib/hotkeys";
import { useHotkeys } from "@/store/hotkeysContext";
import { useLatestRef } from "@/store/useLatestRef";

// Intercepts scratchpad-scope hotkey chords in the capture phase while focus is inside the panel, so
// the shortcut fires before the roster search field or the rich-text editor sees the key. Installed
// once per mount via the passed ref; the live archive handler is read through a ref so the listener
// never re-attaches. `onArchive` is undefined when no scratchpad is open, so the chord is a no-op
// (and passes through) rather than archiving nothing.
export function useScratchpadHotkeys(
  containerRef: RefObject<HTMLElement | null>,
  onArchive: (() => void) | undefined,
): void {
  const { bindings } = useHotkeys();

  const bindingsRef = useLatestRef(bindings);
  const onArchiveRef = useLatestRef(onArchive);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    function handleKey(event: KeyboardEvent) {
      const pressed = bindingFromEvent(event);
      if (!pressed) return;

      const action = matchHotkey(bindingsRef.current, "scratchpad", pressed);
      if (action !== "archive_scratchpad") return;

      const archive = onArchiveRef.current;
      if (!archive) return;
      archive();
      event.preventDefault();
      event.stopPropagation();
    }

    el.addEventListener("keydown", handleKey, { capture: true });
    return () => el.removeEventListener("keydown", handleKey, { capture: true });
  }, [containerRef, bindingsRef, onArchiveRef]); // all stable refs — attach once, read live values through them
}
