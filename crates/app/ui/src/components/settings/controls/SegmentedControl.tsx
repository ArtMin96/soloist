import { ToggleGroup } from "radix-ui";
import type { Option } from "@/lib/appearance";
import { cn } from "@/lib/utils";

// A flat segmented single-select: the active segment lifts to the content surface over the
// muted track (tonal layering, no shadow). Built on Radix ToggleGroup for roving-tabindex
// keyboard nav and correct single-select semantics; a selection can never be cleared to empty.
export function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
}: {
  value: T;
  options: Option<T>[];
  onChange: (value: T) => void;
  ariaLabel: string;
}) {
  return (
    <ToggleGroup.Root
      type="single"
      value={value}
      onValueChange={(next) => {
        if (next) onChange(next as T);
      }}
      aria-label={ariaLabel}
      className="inline-flex rounded-lg border border-border bg-muted p-0.5"
    >
      {options.map((option) => (
        <ToggleGroup.Item
          key={option.value}
          value={option.value}
          className={cn(
            "rounded-md px-2.5 py-1 text-[0.8125rem] text-muted-foreground transition-colors",
            "hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-ring",
            "data-[state=on]:bg-background data-[state=on]:text-foreground",
          )}
        >
          {option.label}
        </ToggleGroup.Item>
      ))}
    </ToggleGroup.Root>
  );
}
