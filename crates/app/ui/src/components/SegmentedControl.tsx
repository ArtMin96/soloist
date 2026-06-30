import { ToggleGroup } from "radix-ui";
import type { Option } from "@/lib/appearance";
import { cn } from "@/lib/utils";

// A macOS segmented control: a single-select where the active segment lifts to the content
// surface over a recessed muted track (tonal layering, no shadow — DESIGN.md flat-by-default).
// Built on Radix ToggleGroup for roving-tabindex keyboard nav and correct single-select
// semantics; a selection can never be cleared to empty. `counts` puts a quiet monochrome badge
// on a segment (e.g. the number of armed timers) — never a saturated hue, which stays on status.
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
  return (
    <ToggleGroup.Root
      type="single"
      value={value}
      onValueChange={(next) => {
        if (next) onChange(next as T);
      }}
      aria-label={ariaLabel}
      className="inline-flex items-center rounded-lg border border-border bg-muted p-0.5"
    >
      {options.map((option) => {
        const count = counts?.[option.value];
        return (
          <ToggleGroup.Item
            key={option.value}
            value={option.value}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[0.8125rem] text-muted-foreground transition-colors",
              "hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring",
              "data-[state=on]:bg-background data-[state=on]:text-foreground",
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
