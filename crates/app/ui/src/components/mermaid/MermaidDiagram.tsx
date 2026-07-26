import { useEffect, useState } from "react";
import { TriangleAlert } from "lucide-react";
import { renderDiagram } from "@/lib/mermaid/engine";
import { useMermaidTheme } from "@/lib/mermaid/useMermaidTheme";
import { useLatestRef } from "@/store/useLatestRef";
import { cn } from "@/lib/utils";
import "./mermaid.css";

export interface MermaidDiagramProps {
  /** The diagram's Mermaid source. */
  source: string;
  className?: string;
  /** Reports whether the current source rendered — the host uses it to react to a broken diagram. */
  onParse?: (ok: boolean) => void;
}

interface RenderState {
  /** The most recent SVG that drew, kept mounted while a newer render is in flight. */
  svg: string | null;
  /** The failure the current source produced, or null. */
  message: string | null;
  /** True while a render is outstanding. */
  pending: boolean;
}

const INITIAL: RenderState = { svg: null, message: null, pending: true };

/**
 * Renders Mermaid `source` to a diagram, self-contained enough to drop into any surface (the editor's
 * code-block NodeView, the diagrams panel): it holds no editing concern and only shows what the source
 * produces. It re-renders whenever the source or the app theme changes, and a result from a superseded
 * render is discarded so a slow render can never overwrite a newer one.
 *
 * A re-render keeps the previous diagram on screen, dimmed, rather than clearing to a skeleton — a
 * Mermaid render takes long enough that swapping to a placeholder reads as the diagram disappearing
 * every time a theme is picked or an edit settles. The skeleton is for the first render only, when
 * there is genuinely nothing to show.
 */
export function MermaidDiagram({ source, className, onParse }: MermaidDiagramProps) {
  const signature = useMermaidTheme();
  const onParseRef = useLatestRef(onParse);
  const [state, setState] = useState<RenderState>(INITIAL);

  useEffect(() => {
    let active = true;
    setState((current) => ({ ...current, pending: true }));
    void renderDiagram(source).then((result) => {
      if (!active) return;
      const ok = "svg" in result;
      setState((current) => ({
        svg: ok ? result.svg : current.svg,
        message: ok ? null : result.error,
        pending: false,
      }));
      onParseRef.current?.(ok);
    });
    return () => {
      active = false;
    };
  }, [source, signature, onParseRef]);

  // The mounted SVG no longer reflects the current source: either a newer render is in flight, or the
  // current source is broken and this is the last one that drew. Dimming says so without hiding it.
  const stale = state.svg !== null && (state.pending || state.message !== null);
  return (
    <div className={cn("mermaid-surface", className)}>
      {state.pending && state.svg === null && !state.message && (
        <div className="mermaid-loading" data-testid="mermaid-skeleton" aria-hidden />
      )}
      {state.message !== null && (
        <div className="mermaid-error" role="alert">
          <TriangleAlert className="mermaid-error-icon" aria-hidden />
          <span className="mermaid-error-message">{state.message}</span>
        </div>
      )}
      {state.svg !== null && (
        // The SVG is sanitized by Mermaid's strict security level before it is returned, so injecting
        // it as markup is safe — there is no other way to mount server-rendered SVG markup.
        <div
          className={cn("mermaid-rendered", stale && "is-stale")}
          aria-busy={stale || undefined}
          dangerouslySetInnerHTML={{ __html: state.svg }}
        />
      )}
    </div>
  );
}
