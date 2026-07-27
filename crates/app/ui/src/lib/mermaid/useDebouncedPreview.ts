// The wait between editing a diagram's source and previewing it.

import { useCallback, useEffect, useRef, useState } from "react";
import { useLatestRef } from "@/store/useLatestRef";
import { MERMAID_RENDER_DEBOUNCE_MS } from "./const";

/**
 * A debounced copy of `source` to preview, and the edit handler that arms the debounce.
 *
 * The wait exists to coalesce keystrokes, so only a change made through the returned handler waits it
 * out. Every other way the source can change — the header's theme picker rewriting the frontmatter, a
 * document arriving from disk — is one discrete change with nothing to coalesce, and it previews at
 * once; making someone watch a debounce before a render that itself takes half a second is most of why
 * picking a theme felt slow.
 *
 * The two are told apart by the exact text an edit produced, not by whether an edit happened recently.
 * That distinction is the point: a theme picked while a keystroke's debounce is still outstanding is
 * not typing, and a rule phrased in terms of recency waits it out anyway — the very delay this removes.
 */
export function useDebouncedPreview(
  source: string,
  onChange: (next: string) => void,
): [string, (next: string) => void] {
  const [preview, setPreview] = useState(source);
  const onChangeRef = useLatestRef(onChange);
  /** The text the last edit produced, or null once something else has set the source. */
  const typed = useRef<string | null>(null);

  useEffect(() => {
    if (source !== typed.current) {
      typed.current = null;
      setPreview(source);
      return;
    }
    const id = setTimeout(() => setPreview(source), MERMAID_RENDER_DEBOUNCE_MS);
    return () => clearTimeout(id);
  }, [source]);

  const edit = useCallback(
    (next: string) => {
      typed.current = next;
      onChangeRef.current(next);
    },
    [onChangeRef],
  );

  return [preview, edit];
}
