import { useCallback, useRef, type PointerEvent as ReactPointerEvent } from "react";
import { clampSize } from "@/store/useStoredLayout";
import { cn } from "@/lib/utils";

/** Which way a divider runs, and therefore which measurement dragging it changes. */
export type PaneDividerOrientation = "vertical" | "horizontal";

interface PaneDividerProps {
  /** `vertical` draws a vertical line and sizes a width; `horizontal` sizes a height. */
  orientation: PaneDividerOrientation;
  /** Names the divider for a screen reader; also its tooltip. */
  label: string;
  size: number;
  min: number;
  max: number;
  /** How far one arrow-key press moves it. */
  step: number;
  onResize: (size: number) => void;
}

/**
 * The draggable edge of a docked pane, sized by pointer or by arrow key.
 *
 * A separator of its own rather than the app's `ResizablePanelGroup`: the group would have to
 * wrap the panes it sits between — which for either of this app's two docked panes means
 * wrapping the terminal, moving a layout library into the bundle every launch pays for and
 * rebuilding the emulator when the wrapper resolves. Both panes it serves load lazily, so
 * everything here loads with them.
 *
 * Both panes are docked to the far edge of their container, so in each case dragging *toward*
 * the content grows the pane — which is why one component covers both.
 */
export function PaneDivider({
  orientation,
  label,
  size,
  min,
  max,
  step,
  onResize,
}: PaneDividerProps) {
  const vertical = orientation === "vertical";
  const dragging = useRef<{ from: number; size: number } | null>(null);

  const onPointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      // The primary button only: a right-click here belongs to the context menu, not to a drag.
      if (event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      dragging.current = { from: vertical ? event.clientX : event.clientY, size };
    },
    [size, vertical],
  );

  const onPointerMove = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      const start = dragging.current;
      if (start === null) return;
      const now = vertical ? event.clientX : event.clientY;
      onResize(clampSize(start.size + (start.from - now), min, max));
    },
    [max, min, onResize, vertical],
  );

  const endDrag = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (dragging.current === null) return;
    dragging.current = null;
    event.currentTarget.releasePointerCapture(event.pointerId);
  }, []);

  const grow = vertical ? "ArrowLeft" : "ArrowUp";
  const shrink = vertical ? "ArrowRight" : "ArrowDown";

  return (
    <div
      role="separator"
      aria-orientation={orientation}
      aria-label={label}
      title={label}
      aria-valuenow={size}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={(event) => {
        const move = event.key === grow ? step : event.key === shrink ? -step : 0;
        if (move === 0) return;
        event.preventDefault();
        onResize(clampSize(size + move, min, max));
      }}
      // A hairline that widens its *hit* area without widening its look: the visible line stays
      // 1px (structure is drawn with hairlines), while the transparent inset gives the pointer
      // something to catch.
      className={cn(
        "relative shrink-0 bg-sidebar-border outline-none hover:bg-ring focus-visible:bg-ring",
        vertical
          ? "w-px cursor-col-resize after:absolute after:inset-y-0 after:-inset-x-1"
          : "h-px cursor-row-resize after:absolute after:inset-x-0 after:-inset-y-1",
      )}
    />
  );
}
