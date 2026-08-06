import { useCallback, useRef, type PointerEvent as ReactPointerEvent } from "react";
import {
  clampWidth,
  RAIL_MAX_WIDTH,
  RAIL_MIN_WIDTH,
  RAIL_RESIZE_STEP,
} from "@/store/git/useRailLayout";

/** Names the divider for a screen reader; also its tooltip. */
const LABEL = "Resize the version control rail";

interface RailDividerProps {
  width: number;
  onResize: (width: number) => void;
}

/**
 * The rail's leading edge, dragged or arrowed to set its width.
 *
 * A separator of its own rather than the app's `ResizablePanelGroup`: the group would have to
 * wrap the terminal pane too, which would move a layout library into the bundle every launch
 * pays for, to resize a panel most launches never open. Everything here loads with the rail.
 */
export function RailDivider({ width, onResize }: RailDividerProps) {
  const dragging = useRef<{ x: number; width: number } | null>(null);

  const onPointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      // The primary button only: a right-click here belongs to the context menu, not to a drag.
      if (event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      dragging.current = { x: event.clientX, width };
    },
    [width],
  );

  const onPointerMove = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      const start = dragging.current;
      if (start === null) return;
      // The rail is docked to the trailing edge, so dragging left widens it.
      onResize(clampWidth(start.width + (start.x - event.clientX)));
    },
    [onResize],
  );

  const endDrag = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (dragging.current === null) return;
    dragging.current = null;
    event.currentTarget.releasePointerCapture(event.pointerId);
  }, []);

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={LABEL}
      title={LABEL}
      aria-valuenow={width}
      aria-valuemin={RAIL_MIN_WIDTH}
      aria-valuemax={RAIL_MAX_WIDTH}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onKeyDown={(event) => {
        const step =
          event.key === "ArrowLeft"
            ? RAIL_RESIZE_STEP
            : event.key === "ArrowRight"
              ? -RAIL_RESIZE_STEP
              : 0;
        if (step === 0) return;
        event.preventDefault();
        onResize(clampWidth(width + step));
      }}
      // A hairline that widens its *hit* area without widening its look: the visible line stays
      // 1px (structure is drawn with hairlines), while the transparent inset gives the pointer
      // something to catch.
      className="relative w-px shrink-0 cursor-col-resize bg-sidebar-border outline-none after:absolute after:inset-y-0 after:-inset-x-1 hover:bg-ring focus-visible:bg-ring"
    />
  );
}
