import { STATUS } from "@/lib/status";
import { cn } from "@/lib/utils";
import type { ProcStatus } from "@/domain";

interface StatusIndicatorProps {
  status: ProcStatus;
  /** Show the text label beside the glyph. Off in the dense sidebar (tooltip carries it). */
  showLabel?: boolean;
}

// The product's heartbeat: a process status rendered as glyph + colored dot + label.
// The glyph carries the state without color, the hue reinforces it, the label names it —
// so status survives color blindness and a grayscale screenshot (DESIGN.md). The
// `data-status` attribute keys styling and tests off the value, not scraped text.
export function StatusIndicator({ status, showLabel = true }: StatusIndicatorProps) {
  const { label, glyph, toneClass, transitional } = STATUS[status];
  return (
    <span
      data-status={status}
      title={label}
      className="inline-flex items-center gap-1.5 leading-none"
    >
      <span
        aria-hidden
        className={cn(
          "text-[0.7rem] leading-none",
          toneClass,
          transitional && "motion-safe:animate-pulse",
        )}
      >
        {glyph}
      </span>
      {showLabel ? (
        <span className="text-xs text-muted-foreground">{label}</span>
      ) : (
        <span className="sr-only">{label}</span>
      )}
    </span>
  );
}
