import { Suspense, useState, type ReactNode } from "react";

// Mounts a heavy overlay only after it has first been opened, so its code-split chunk (and its
// dependencies) stay out of the initial bundle until the user reaches for it. Once opened it
// stays mounted, so the overlay's own open/close transitions keep animating — only the very
// first open waits on the chunk, which loads from local disk and so arrives effectively at once.
export function DeferredOverlay({ open, children }: { open: boolean; children: ReactNode }) {
  // A monotonic "has it ever been open?" latch. Adjusting it during render (not in an effect)
  // commits the mount in the same pass the open flips, with no stale null frame in between.
  const [mounted, setMounted] = useState(false);
  if (open && !mounted) setMounted(true);
  if (!mounted) return null;
  return <Suspense fallback={null}>{children}</Suspense>;
}
