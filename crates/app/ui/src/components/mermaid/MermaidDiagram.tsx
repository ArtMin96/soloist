import { TriangleAlert } from "lucide-react";
import { useDiagramRender } from "@/lib/mermaid/useDiagramRender";
import { cn } from "@/lib/utils";
import "./mermaid.css";

export interface MermaidDiagramProps {
  /** The diagram's Mermaid source. */
  source: string;
  className?: string;
  /** Reports whether the current source rendered — the host uses it to react to a broken diagram. */
  onParse?: (ok: boolean) => void;
}

/**
 * Renders Mermaid `source` to a diagram, self-contained enough to drop into any surface (the editor's
 * code-block NodeView, the diagrams panel): it holds no editing concern and only shows what the source
 * produces. What to draw, and when, is `useDiagramRender`; this shows the three states it reports.
 *
 * The skeleton is for the first render only. Once a diagram has drawn it stays on screen and dims
 * while a newer render runs or while the current source is broken — a Mermaid render takes long enough
 * that clearing to a placeholder reads as the diagram disappearing every time an edit settles.
 */
export function MermaidDiagram({ source, className, onParse }: MermaidDiagramProps) {
  const render = useDiagramRender(source, onParse);
  // The mounted SVG no longer reflects the current source: either a newer render is in flight, or the
  // current source is broken and this is the last one that drew. Dimming says so without hiding it.
  const stale = render.drawn !== null && render.status !== "drawn";
  return (
    <div className={cn("mermaid-surface", className)}>
      {render.status === "pending" && render.drawn === null && (
        <div className="mermaid-loading" data-testid="mermaid-skeleton" aria-hidden />
      )}
      {render.status === "error" && (
        <div className="mermaid-error" role="alert">
          <TriangleAlert className="mermaid-error-icon" aria-hidden />
          <span className="mermaid-error-message">{render.message}</span>
        </div>
      )}
      {render.drawn !== null && (
        // The SVG is sanitized by Mermaid's strict security level before it is returned, so injecting
        // it as markup is safe — there is no other way to mount server-rendered SVG markup.
        <div
          className={cn("mermaid-rendered", stale && "is-stale")}
          aria-busy={stale || undefined}
          dangerouslySetInnerHTML={{ __html: render.drawn }}
        />
      )}
    </div>
  );
}
