import {
  useCallback,
  useRef,
  useState,
  type PointerEvent,
  type ReactNode,
  type WheelEvent,
} from "react";
import { Maximize, RotateCcw, ZoomIn, ZoomOut } from "lucide-react";
import { MermaidDiagram } from "@/components/mermaid/MermaidDiagram";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import {
  clampZoom,
  IDENTITY_TRANSFORM,
  MERMAID_ZOOM_STEP,
  zoomAround,
  type Transform,
} from "@/lib/mermaid";
import { cn } from "@/lib/utils";
import "./diagramCanvas.css";

// Padding kept around a fitted diagram so it never touches the canvas edges.
const FIT_PADDING = 24;

interface DiagramCanvasProps {
  /** The Mermaid source to render and let the user zoom/pan. */
  source: string;
  className?: string;
  /** Forwarded from the inner renderer — whether the current source rendered. */
  onParse?: (ok: boolean) => void;
}

// A reusable pan-and-zoom viewport around the shared `MermaidDiagram` renderer: wheel zooms to the
// cursor, drag pans, and a slim overlay toolbar offers zoom in/out, fit-to-view, and reset. The
// transform math is the pure `lib/mermaid/zoom` module; this component owns only the pointer/wheel
// wiring and the measured fit. Presentational and self-contained — it holds no editing or document
// concern, so the editor's preview and (later) the code-block NodeView can both mount it. The diagram
// is fit to the viewport on its first render and then holds the user's view across source edits.
export function DiagramCanvas({ source, className, onParse }: DiagramCanvasProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [transform, setTransform] = useState<Transform>(IDENTITY_TRANSFORM);
  const [panning, setPanning] = useState(false);
  const pan = useRef<{ pointerId: number; startX: number; startY: number } | null>(null);
  // True once the user has zoomed or panned — auto-fit only runs while the view is still untouched, so
  // a source edit re-renders without yanking the view back.
  const adjusted = useRef(false);

  const fit = useCallback(() => {
    const viewport = viewportRef.current;
    const content = contentRef.current;
    if (!viewport || !content) return;
    // A CSS transform does not change layout size, so `offsetWidth/Height` is the unscaled diagram.
    const w = content.offsetWidth;
    const h = content.offsetHeight;
    if (!w || !h) return;
    const availW = viewport.clientWidth - FIT_PADDING;
    const availH = viewport.clientHeight - FIT_PADDING;
    const scale = clampZoom(Math.min(availW / w, availH / h, 1));
    setTransform({
      scale,
      x: (viewport.clientWidth - w * scale) / 2,
      y: (viewport.clientHeight - h * scale) / 2,
    });
  }, []);

  const reset = useCallback(() => {
    adjusted.current = true;
    setTransform(IDENTITY_TRANSFORM);
  }, []);

  const zoomByStep = useCallback((direction: 1 | -1) => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    adjusted.current = true;
    const factor = direction === 1 ? 1 + MERMAID_ZOOM_STEP : 1 / (1 + MERMAID_ZOOM_STEP);
    setTransform((current) =>
      zoomAround(current, factor, viewport.clientWidth / 2, viewport.clientHeight / 2),
    );
  }, []);

  const onWheel = useCallback((event: WheelEvent<HTMLDivElement>) => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    event.preventDefault();
    adjusted.current = true;
    const rect = viewport.getBoundingClientRect();
    const factor = event.deltaY < 0 ? 1 + MERMAID_ZOOM_STEP : 1 / (1 + MERMAID_ZOOM_STEP);
    setTransform((current) =>
      zoomAround(current, factor, event.clientX - rect.left, event.clientY - rect.top),
    );
  }, []);

  const onPointerDown = useCallback((event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    adjusted.current = true;
    pan.current = { pointerId: event.pointerId, startX: event.clientX, startY: event.clientY };
    event.currentTarget.setPointerCapture(event.pointerId);
    setPanning(true);
  }, []);

  const onPointerMove = useCallback((event: PointerEvent<HTMLDivElement>) => {
    const active = pan.current;
    if (!active || active.pointerId !== event.pointerId) return;
    const dx = event.clientX - active.startX;
    const dy = event.clientY - active.startY;
    active.startX = event.clientX;
    active.startY = event.clientY;
    setTransform((current) => ({ ...current, x: current.x + dx, y: current.y + dy }));
  }, []);

  const endPan = useCallback((event: PointerEvent<HTMLDivElement>) => {
    if (pan.current?.pointerId !== event.pointerId) return;
    pan.current = null;
    setPanning(false);
  }, []);

  const handleParse = useCallback(
    (ok: boolean) => {
      onParse?.(ok);
      // Fit the freshly rendered diagram once, only while the user has not taken over the view.
      if (ok && !adjusted.current) requestAnimationFrame(fit);
    },
    [onParse, fit],
  );

  return (
    <div
      ref={viewportRef}
      className={cn("diagram-canvas", panning && "is-panning", className)}
      onWheel={onWheel}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endPan}
      onPointerCancel={endPan}
    >
      <div
        ref={contentRef}
        className="diagram-canvas-content"
        style={{
          transform: `translate(${transform.x}px, ${transform.y}px) scale(${transform.scale})`,
        }}
      >
        <MermaidDiagram source={source} onParse={handleParse} />
      </div>

      <TooltipProvider>
        <div className="diagram-canvas-toolbar" role="toolbar" aria-label="Diagram view">
          <CanvasButton label="Zoom in" onClick={() => zoomByStep(1)}>
            <ZoomIn aria-hidden />
          </CanvasButton>
          <CanvasButton label="Zoom out" onClick={() => zoomByStep(-1)}>
            <ZoomOut aria-hidden />
          </CanvasButton>
          <CanvasButton label="Fit to view" onClick={fit}>
            <Maximize aria-hidden />
          </CanvasButton>
          <CanvasButton label="Reset zoom" onClick={reset}>
            <RotateCcw aria-hidden />
          </CanvasButton>
        </div>
      </TooltipProvider>
    </div>
  );
}

function CanvasButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon-sm" aria-label={label} onClick={onClick}>
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
