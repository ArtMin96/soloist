import type { ReactNode } from "react";
import { LoadingStandIn } from "@/components/common/LoadingStandIn";
import { RecoveryNotice } from "@/components/common/RecoveryNotice";
import { LoadStatus, type Loadable } from "@/store/loadable";

interface LoadableRegionProps<T> {
  state: Loadable<T>;
  /** Names what is loading, for assistive tech and the recovery notice: "todos" → "Loading todos". */
  label: string;
  /** The layout stand-in shown while the first read is in flight. Purely visual; the region owns the loading semantics. */
  skeleton: ReactNode;
  /** Re-runs the failed read; without it the notice has no action. */
  onRetry?: () => void;
  /** Layout for the loading and failed wrappers only (the ready branch renders `children` bare). */
  className?: string;
  children: (value: T) => ReactNode;
}

/**
 * The one place a read model's loading, failure and content are rendered, so every data-bearing
 * surface waits, fails and recovers the same way instead of each inventing a loading branch.
 *
 * The stand-in is revealed on a delay: a read that lands fast would otherwise flash a skeleton and
 * replace it within a frame or two, which reads as a glitch rather than as progress. Only the
 * loading and failed branches draw a wrapper — a held value renders bare, so a region's own layout
 * is whatever it was before it was made loadable.
 */
export function LoadableRegion<T>({
  state,
  label,
  skeleton,
  onRetry,
  className,
  children,
}: LoadableRegionProps<T>): ReactNode {
  switch (state.status) {
    case LoadStatus.Loading:
      return (
        <LoadingStandIn label={label} className={className}>
          {skeleton}
        </LoadingStandIn>
      );
    case LoadStatus.Ready:
      return children(state.value);
    case LoadStatus.Failed:
      return (
        <div className={className}>
          <RecoveryNotice message={`Could not load ${label}.`} onRetry={onRetry} />
        </div>
      );
  }
}
