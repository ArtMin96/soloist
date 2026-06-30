import { ToggleGroup } from "radix-ui";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Option } from "@/lib/appearance";
import { cn } from "@/lib/utils";

// A macOS segmented control: a single-select where one lifted "thumb" slides to the active
// segment over a recessed muted track (tonal layering, no shadow beyond a hair of lift — the
// DESIGN.md flat-by-default rule). The thumb is a single element translated to the measured
// position of the active item, so variable-width labels stay aligned and only the thumb moves
// (the labels never reflow). Built on Radix ToggleGroup for roving-tabindex keyboard nav and
// correct single-select semantics; a selection can never be cleared to empty. `counts` puts a
// quiet monochrome badge on a segment — never a saturated hue, which stays on status.
export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  counts,
}: {
  value: T;
  options: Option<T>[];
  onChange: (value: T) => void;
  ariaLabel: string;
  counts?: Partial<Record<T, number>>;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef(new Map<T, HTMLButtonElement>());
  const [thumb, setThumb] = useState<{ x: number; w: number } | null>(null);
  // Suppress the slide on the very first measured paint (and after a reduced-motion reset);
  // it turns on one frame later so the initial thumb appears in place, not gliding from zero.
  const [animated, setAnimated] = useState(false);

  useLayoutEffect(() => {
    const root = rootRef.current;
    const active = itemRefs.current.get(value);
    if (!root || !active) return;
    const measure = () => {
      const node = itemRefs.current.get(value);
      if (node) setThumb({ x: node.offsetLeft, w: node.offsetWidth });
    };
    measure();
    // Re-measure on container/segment resize (font scale change, count badge appearing) so the
    // thumb tracks the active segment without a layout listener per call site.
    const observer = new ResizeObserver(measure);
    observer.observe(root);
    itemRefs.current.forEach((node) => observer.observe(node));
    return () => observer.disconnect();
  }, [value, options]);

  useEffect(() => {
    if (thumb) setAnimated(true);
  }, [thumb]);

  return (
    <ToggleGroup.Root
      ref={rootRef}
      type="single"
      value={value}
      onValueChange={(next) => {
        if (next) onChange(next as T);
      }}
      aria-label={ariaLabel}
      className="relative inline-flex items-center rounded-lg border border-border bg-muted p-0.5"
    >
      <span
        aria-hidden
        className={cn(
          "pointer-events-none absolute top-0.5 bottom-0.5 left-0 rounded-md bg-background shadow-[0_1px_2px_-1px_oklch(0.2_0.02_255_/_0.25)]",
          animated &&
            "transition-[transform,width] duration-[var(--dur-control)] ease-spring-settle",
          thumb ? "opacity-100" : "opacity-0",
        )}
        style={thumb ? { transform: `translateX(${thumb.x}px)`, width: thumb.w } : undefined}
      />
      {options.map((option) => {
        const count = counts?.[option.value];
        return (
          <ToggleGroup.Item
            key={option.value}
            value={option.value}
            ref={(node) => {
              if (node) itemRefs.current.set(option.value, node);
              else itemRefs.current.delete(option.value);
            }}
            className={cn(
              "relative z-10 inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[0.8125rem] text-muted-foreground",
              "transition-colors duration-[var(--dur-fast)] outline-none",
              "hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring",
              "data-[state=on]:text-foreground",
            )}
          >
            {option.label}
            {count != null && count > 0 && (
              <span className="rounded-full bg-foreground/10 px-1.5 text-[0.6875rem] tabular-nums text-muted-foreground">
                {count}
              </span>
            )}
          </ToggleGroup.Item>
        );
      })}
    </ToggleGroup.Root>
  );
}
