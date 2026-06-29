import { cn } from "@/lib/utils";
import type { ScratchpadSummary } from "@/domain";

interface ScratchpadListProps {
  scratchpads: ScratchpadSummary[];
  selected: string | null;
  onSelect: (name: string) => void;
}

// The scratchpad roster: one row per shared document, its name over a one-line objective gist with
// its revision in mono. Presentational — selection state and the choice arrive as props. Mirrors the
// sidebar ProcessRow idiom (h-auto rows, raised fill + a full-height azure selection marker, never a
// decorative side-stripe).
export function ScratchpadList({ scratchpads, selected, onSelect }: ScratchpadListProps) {
  if (scratchpads.length === 0) {
    return (
      <p className="px-3 py-6 text-[0.8125rem] leading-relaxed text-muted-foreground">
        No scratchpads yet. Agents create them to share a plan or research as they work — they will
        appear here live.
      </p>
    );
  }

  return (
    <ul role="listbox" aria-label="Scratchpads" className="flex flex-col gap-px p-1">
      {scratchpads.map((pad) => {
        const isSelected = pad.name === selected;
        return (
          <li key={pad.id}>
            <button
              type="button"
              role="option"
              aria-selected={isSelected}
              onClick={() => onSelect(pad.name)}
              className={cn(
                "relative flex w-full flex-col gap-0.5 rounded-sm py-1.5 pr-2.5 pl-2.5 text-left outline-none",
                "hover:bg-sidebar-accent focus-visible:bg-sidebar-accent focus-visible:ring-2 focus-visible:ring-sidebar-ring",
                isSelected && "bg-sidebar-accent",
              )}
            >
              {isSelected && (
                <span
                  aria-hidden
                  className="absolute top-1.5 bottom-1.5 left-0 w-0.5 rounded-full bg-sidebar-primary"
                />
              )}
              <span className="flex items-baseline gap-2">
                <span className="min-w-0 flex-1 truncate text-[0.8125rem] text-foreground">
                  {pad.name}
                </span>
                <span className="shrink-0 font-mono text-[0.6875rem] text-muted-foreground/70">
                  r{pad.revision}
                </span>
              </span>
              {pad.objective && (
                <span className="truncate text-[0.6875rem] text-muted-foreground">
                  {pad.objective}
                </span>
              )}
            </button>
          </li>
        );
      })}
    </ul>
  );
}
