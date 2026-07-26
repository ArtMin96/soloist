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

type RenderState =
  | { status: "loading" }
  | { status: "rendered"; svg: string }
  | { status: "error"; message: string };

/**
 * Renders Mermaid `source` to a diagram, self-contained enough to drop into any surface (the editor's
 * code-block NodeView today, a standalone diagrams panel later): it holds no editing concern and only
 * shows what the source produces. It re-renders whenever the source or the app theme changes, and a
 * result from a superseded render is discarded so a slow render can never overwrite a newer one.
 */
export function MermaidDiagram({ source, className, onParse }: MermaidDiagramProps) {
  const signature = useMermaidTheme();
  const onParseRef = useLatestRef(onParse);
  const [state, setState] = useState<RenderState>({ status: "loading" });

  useEffect(() => {
    let active = true;
    setState({ status: "loading" });
    void renderDiagram(source).then((result) => {
      if (!active) return;
      if ("svg" in result) {
        setState({ status: "rendered", svg: result.svg });
        onParseRef.current?.(true);
      } else {
        setState({ status: "error", message: result.error });
        onParseRef.current?.(false);
      }
    });
    return () => {
      active = false;
    };
  }, [source, signature, onParseRef]);

  return (
    <div className={cn("mermaid-surface", className)}>
      {state.status === "loading" && (
        <div className="mermaid-loading" data-testid="mermaid-skeleton" aria-hidden />
      )}
      {state.status === "error" && (
        <div className="mermaid-error" role="alert">
          <TriangleAlert className="mermaid-error-icon" aria-hidden />
          <span className="mermaid-error-message">{state.message}</span>
        </div>
      )}
      {state.status === "rendered" && (
        // The SVG is sanitized by Mermaid's strict security level before it is returned, so injecting
        // it as markup is safe — there is no other way to mount server-rendered SVG markup.
        <div className="mermaid-rendered" dangerouslySetInnerHTML={{ __html: state.svg }} />
      )}
    </div>
  );
}
