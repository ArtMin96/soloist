// Driving one diagram surface's rendering: what it should show, and how many renders it is allowed to
// have outstanding while it works that out.

import { useCallback, useEffect, useRef, useState } from "react";
import { useLatestRef } from "@/store/useLatestRef";
import { renderDiagram } from "./engine";
import { useMermaidTheme } from "./useMermaidTheme";

/**
 * What a diagram surface has to show, as one closed set of states.
 *
 * `drawn` is the last SVG that rendered, and it deliberately outlives the source that produced it: a
 * Mermaid render is slow enough that clearing the pane while the next one runs reads as the diagram
 * vanishing, so a superseded or broken source keeps the previous diagram up (dimmed) instead. Pairing
 * that with the current outcome is what the three states say. Independently nullable fields would spell
 * the same thing while also being able to spell combinations that mean nothing.
 */
export type DiagramRender =
  | { status: "pending"; drawn: string | null }
  | { status: "drawn"; drawn: string }
  | { status: "error"; drawn: string | null; message: string };

const FIRST_RENDER: DiagramRender = { status: "pending", drawn: null };

/** A render this surface wants: the source to draw, and the palette that asked for it. */
interface Request {
  source: string;
  signature: string;
}

/**
 * Render `source` for one surface, re-rendering when the source or the app palette changes.
 *
 * The surface keeps **at most one render outstanding**. While one is drawing, a newer request replaces
 * any older one still waiting rather than joining a queue behind it: a superseded source would cost a
 * full Mermaid render — half a second, the expensive part — to produce output nothing will ever show.
 * Left to accumulate they also outrun the renderer, since an edit can arrive every debounce interval
 * while a draw takes several of them, and the backlog then keeps drawing long after typing has stopped.
 * This is also the ceiling on the engine's render queue, and the only place one can be applied: the
 * engine cannot tell a superseded render from an export that must always run.
 */
export function useDiagramRender(source: string, onParse?: (ok: boolean) => void): DiagramRender {
  const signature = useMermaidTheme();
  const onParseRef = useLatestRef(onParse);
  const [render, setRender] = useState<DiagramRender>(FIRST_RENDER);

  const next = useRef<Request | null>(null);
  const drawing = useRef(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const drain = useCallback(async () => {
    try {
      let request = next.current;
      while (request !== null) {
        next.current = null;
        const result = await renderDiagram(request.source);
        if (!mounted.current) return;
        // A newer request arrived while this one drew, so it is already stale. Showing it would put a
        // diagram on screen that the current source no longer describes, and report its validity.
        request = next.current;
        if (request !== null) continue;
        const ok = "svg" in result;
        setRender((current) =>
          ok
            ? { status: "drawn", drawn: result.svg }
            : { status: "error", drawn: current.drawn, message: result.error },
        );
        onParseRef.current?.(ok);
      }
    } finally {
      drawing.current = false;
    }
  }, [onParseRef]);

  // A source or palette change shows as pending at once — adjusted directly during render (the
  // documented pattern for resetting state when an input changes: react.dev/reference/react/useState
  // #storing-information-from-previous-renders), rather than a tick later once an effect runs.
  const [requestedFor, setRequestedFor] = useState<Request>({ source, signature });
  if (requestedFor.source !== source || requestedFor.signature !== signature) {
    setRequestedFor({ source, signature });
    setRender((current) => ({ status: "pending", drawn: current.drawn }));
  }

  useEffect(() => {
    next.current = { source, signature };
    if (drawing.current) return;
    drawing.current = true;
    void drain();
  }, [source, signature, drain]);

  return render;
}
