import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface LoadingStandInProps {
  /**
   * Names what is loading for assistive tech ("description" → "Loading description"). Omit it for a
   * stand-in inside a structure that already reads — a comment body under its author line — which
   * then carries only `aria-busy`: a thread of ten bodies must not announce ten waits.
   */
  label?: string;
  className?: string;
  /** The purely visual stand-in; rendered `aria-hidden`. */
  children: ReactNode;
}

/**
 * The one wrapper that says "this is not the content yet": it marks the box busy, names the wait
 * once for a screen reader, hides the drawing itself from the accessibility tree, and reveals it
 * only after the shared delay — so a read that lands inside that window never flashes a stand-in.
 *
 * Reveal timing and the announcement live here alone, so a region and an inline body wait the same
 * way rather than each inventing a loading treatment.
 */
export function LoadingStandIn({ label, className, children }: LoadingStandInProps) {
  return (
    <div
      role={label ? "status" : undefined}
      aria-busy="true"
      className={cn("animate-skeleton-reveal", className)}
    >
      {label && <span className="sr-only">Loading {label}</span>}
      <div aria-hidden>{children}</div>
    </div>
  );
}
