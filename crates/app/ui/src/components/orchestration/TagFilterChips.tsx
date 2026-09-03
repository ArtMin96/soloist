import { Toggle } from "@/components/ui/toggle";
import { cn } from "@/lib/utils";

interface TagFilterChipsProps {
  tags: string[];
  active: string | null;
  onToggle: (tag: string | null) => void;
}

// A row of toggleable tag chips shared by every document/todo filter bar — clicking the active one
// clears the filter. Renders nothing when there are no tags to filter by. Each chip is a shadcn
// `Toggle` rather than a hand-rolled button, so `aria-pressed` and the disabled/focus-visible
// treatment come from Radix; the row itself stays a plain `role="group"` div rather than a Radix
// `ToggleGroup`, because `ToggleGroup` type="single" swaps each item's `aria-pressed` for
// `aria-checked`/`role="radio"`, and type="multiple" would misrepresent an at-most-one selection as
// an independent multi-select.
export function TagFilterChips({ tags, active, onToggle }: TagFilterChipsProps) {
  if (tags.length === 0) return null;
  return (
    <div className="flex flex-wrap gap-1" role="group" aria-label="Filter by tag">
      {tags.map((tag) => {
        const isActive = active === tag;
        return (
          <Toggle
            key={tag}
            size="sm"
            pressed={isActive}
            onPressedChange={(pressed) => onToggle(pressed ? tag : null)}
            className={cn(
              "type-label h-5 min-w-0 rounded-full px-2 font-medium transition-colors duration-[var(--dur-fast)] ease-out-quint",
              "focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none",
              // Stacked rather than a single `data-[state=on]:` variant: the base `Toggle` primitive
              // carries its own unconditional `hover:bg-muted` and `aria-pressed:bg-muted`, each at the
              // same specificity as a lone `data-[state=on]:bg-…` rule, so which one paints would depend
              // on Tailwind's stylesheet emit order. Requiring both attributes (always true together on
              // a pressed Radix `Toggle`) raises this rule's specificity above every base selector it
              // could otherwise tie with, so the selected fill always wins.
              "data-[state=on]:aria-pressed:bg-[var(--sidebar-sel-fill)] data-[state=on]:aria-pressed:text-foreground",
              !isActive && "text-muted-foreground hover:bg-sidebar-accent",
            )}
          >
            {tag}
          </Toggle>
        );
      })}
    </div>
  );
}
