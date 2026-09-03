import { Suspense, useState, type ReactNode } from "react";
import { PaneErrorBoundary } from "@/components/PaneErrorBoundary";

// Mounts a heavy overlay only after it has first been opened, so its code-split chunk (and its
// dependencies) stay out of the initial bundle until the user reaches for it. Once opened it
// stays mounted, so the overlay's own open/close transitions keep animating — only the very
// first open waits on the chunk, which loads from local disk and so arrives effectively at once.
// A render error in the overlay, including a failed chunk load, is caught rather than taking
// down the rest of the app — `label` names the overlay in the recovery notice.
export function DeferredOverlay({
  open,
  label,
  children,
}: {
  open: boolean;
  label?: string;
  children: ReactNode;
}) {
  // A monotonic "has it ever been open?" latch. Adjusting it during render (not in an effect)
  // commits the mount in the same pass the open flips, with no stale null frame in between.
  const [mounted, setMounted] = useState(false);
  if (open && !mounted) setMounted(true);
  if (!mounted) return null;
  return (
    <PaneErrorBoundary label={label}>
      <Suspense fallback={null}>{children}</Suspense>
    </PaneErrorBoundary>
  );
}
