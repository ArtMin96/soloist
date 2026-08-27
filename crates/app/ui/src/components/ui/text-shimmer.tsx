import { cn } from "@/lib/utils";

/**
 * A single-line label that reports ongoing work by sweeping a highlight across itself.
 *
 * The text is painted twice: a solid base that is always fully opaque, and an `aria-hidden` copy
 * above it that a narrow travelling mask reveals a slice at a time. Stacking them this way is what
 * keeps the effect legible — the base carries the contrast on its own, and the sweep only ever adds
 * colour on top, so the label never thins into a gradient the way `background-clip: text` would.
 *
 * Under `prefers-reduced-motion` the highlight is dropped rather than parked mid-sweep, leaving the
 * plain solid label.
 */
export function TextShimmer({
  text,
  active,
  highlightClassName,
  className,
  title,
}: {
  text: string;
  /** Whether the work being reported is still in flight. Idle labels render as plain text. */
  active: boolean;
  /** Tone of the travelling highlight, so the colour stays owned by the status vocabulary. */
  highlightClassName?: string;
  className?: string;
  title?: string;
}) {
  return (
    <span className={cn("relative block min-w-0", className)} title={title}>
      <span className="block truncate">{text}</span>
      {active && (
        <span
          data-slot="text-shimmer"
          aria-hidden
          className={cn(
            "text-shimmer-band pointer-events-none absolute inset-0 block truncate",
            "animate-text-shimmer motion-reduce:hidden",
            highlightClassName,
          )}
        >
          {text}
        </span>
      )}
    </span>
  );
}
