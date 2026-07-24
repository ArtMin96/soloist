import { useId, useState, type KeyboardEvent, type ReactNode } from "react";
import { humanizeName } from "@/lib/humanize";
import { cn } from "@/lib/utils";
import type { DiagramSummary } from "@/domain";

interface DiagramListProps {
  diagrams: DiagramSummary[];
  selected: string | null;
  onSelect: (name: string) => void;
  /** The listbox's accessible name — lets a grouped roster label the active vs archived lists apart. */
  label?: string;
  /** Shown in place of the list when it is empty; defaults to the first-run guidance. */
  emptyHint?: ReactNode;
}

// The diagram roster: a single-select ARIA listbox, one row per diagram (its humanized title over a
// one-line source gist with its revision in mono). The row selects by the raw name handle —
// humanization is display only. Arrow keys / Home / End move the roving focus between options; Enter,
// Space, or a click opens the focused diagram. Activation is explicit (opening reads the full source) —
// scan with the arrows, commit with Enter. The option roles ride native <button>s so each is focusable
// and keyboard-operable, and the listbox rides a generic <div> so no list element's semantics are
// overridden. Presentational — selection and the choice arrive as props. The tint-in-place selection is
// the shared macOS source-list language, identical to the scratchpad list and the sidebar ProcessRow.
export function DiagramList({
  diagrams,
  selected,
  onSelect,
  label = "Diagrams",
  emptyHint,
}: DiagramListProps) {
  const baseId = useId();
  // Track the roving cursor by the diagram's name, not its index, so a diagram added or removed live
  // keeps the cursor on the same document instead of sliding onto a neighbour.
  const [activeName, setActiveName] = useState<string | null>(selected);

  if (diagrams.length === 0) {
    return (
      <p className="px-3 py-6 text-[0.8125rem] leading-relaxed text-muted-foreground">
        {emptyHint ?? (
          <>
            No diagrams yet. Agents create them to sketch an architecture or a flow as they work —
            they will appear here live.
          </>
        )}
      </p>
    );
  }

  // Resolve the cursor to a live row; a name whose diagram was removed falls back to the first row.
  const activeIndex = Math.max(
    0,
    diagrams.findIndex((diagram) => diagram.name === activeName),
  );
  const optionId = (index: number) => `${baseId}-option-${index}`;

  function moveTo(index: number) {
    const clamped = Math.max(0, Math.min(index, diagrams.length - 1));
    setActiveName(diagrams[clamped].name);
    document.getElementById(optionId(clamped))?.focus();
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    switch (event.key) {
      case "ArrowDown":
        moveTo(activeIndex + 1);
        break;
      case "ArrowUp":
        moveTo(activeIndex - 1);
        break;
      case "Home":
        moveTo(0);
        break;
      case "End":
        moveTo(diagrams.length - 1);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  return (
    <div
      role="listbox"
      aria-label={label}
      tabIndex={-1}
      onKeyDown={onKeyDown}
      // Marks the list as its own selection scope: its rows are azure-tinted only while the keyboard is
      // in here, and neutral once focus moves to the document (see index.css).
      data-selection-scope
      className="flex flex-col gap-px p-1 outline-none"
    >
      {diagrams.map((diagram, index) => {
        const isSelected = diagram.name === selected;
        return (
          <button
            key={diagram.id}
            id={optionId(index)}
            type="button"
            role="option"
            aria-selected={isSelected}
            // The raw handle the row addresses, kept reachable now that the row reads as prose.
            data-diagram-name={diagram.name}
            // Roving tabindex: only the cursor's option is in the tab order; the arrows move it.
            tabIndex={index === activeIndex ? 0 : -1}
            onClick={() => {
              setActiveName(diagram.name);
              onSelect(diagram.name);
            }}
            className={cn(
              // The source list's default row height, so a one-line row keeps the same rhythm as the
              // sidebar; a row carrying a gist grows to its second line from here.
              "flex min-h-7 w-full flex-col justify-center rounded-md px-2 py-1 text-left outline-none transition-colors duration-[var(--dur-select)] ease-out-quint",
              "focus-visible:ring-2 focus-visible:ring-sidebar-ring",
              isSelected
                ? "bg-[var(--sel-fill)] hover:bg-[var(--sel-fill-hover)]"
                : "hover:bg-sidebar-accent focus-visible:bg-sidebar-accent",
            )}
          >
            <span className="flex items-baseline gap-2">
              <span className="min-w-0 flex-1 truncate text-[0.8125rem] leading-4 text-foreground">
                {humanizeName(diagram.name)}
              </span>
              <span className="type-label shrink-0 font-mono tabular-nums text-muted-foreground">
                r{diagram.revision}
              </span>
            </span>
            {diagram.gist && (
              <span className="type-label truncate text-muted-foreground">{diagram.gist}</span>
            )}
          </button>
        );
      })}
    </div>
  );
}
