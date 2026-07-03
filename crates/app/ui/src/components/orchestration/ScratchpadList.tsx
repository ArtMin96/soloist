import { useId, useState, type KeyboardEvent } from "react";
import { cn } from "@/lib/utils";
import type { ScratchpadSummary } from "@/domain";

interface ScratchpadListProps {
  scratchpads: ScratchpadSummary[];
  selected: string | null;
  onSelect: (name: string) => void;
}

// The scratchpad roster: a single-select ARIA listbox, one row per shared document (its name over a
// one-line objective gist with its revision in mono). Arrow keys / Home / End move the roving focus
// between options; Enter, Space, or a click opens the focused document. Activation is explicit
// (opening reads the full document) — scan with the arrows, commit with Enter. The option roles ride
// native <button>s so each is focusable and keyboard-operable, and the listbox rides a generic <div>
// so no list element's semantics are overridden. Presentational — selection and the choice arrive as
// props. The tint-in-place selection is the shared macOS source-list language, identical to the
// sidebar ProcessRow.
export function ScratchpadList({ scratchpads, selected, onSelect }: ScratchpadListProps) {
  const baseId = useId();
  // Track the roving cursor by the pad's name, not its index, so a scratchpad added or removed live
  // keeps the cursor on the same document instead of sliding onto a neighbour.
  const [activeName, setActiveName] = useState<string | null>(selected);

  if (scratchpads.length === 0) {
    return (
      <p className="px-3 py-6 text-[0.8125rem] leading-relaxed text-muted-foreground">
        No scratchpads yet. Agents create them to share a plan or research as they work — they will
        appear here live.
      </p>
    );
  }

  // Resolve the cursor to a live row; a name whose pad was removed falls back to the first row.
  const activeIndex = Math.max(
    0,
    scratchpads.findIndex((pad) => pad.name === activeName),
  );
  const optionId = (index: number) => `${baseId}-option-${index}`;

  function moveTo(index: number) {
    const clamped = Math.max(0, Math.min(index, scratchpads.length - 1));
    setActiveName(scratchpads[clamped].name);
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
        moveTo(scratchpads.length - 1);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  return (
    <div
      role="listbox"
      aria-label="Scratchpads"
      tabIndex={-1}
      onKeyDown={onKeyDown}
      className="flex flex-col gap-px p-1 outline-none"
    >
      {scratchpads.map((pad, index) => {
        const isSelected = pad.name === selected;
        return (
          <button
            key={pad.id}
            id={optionId(index)}
            type="button"
            role="option"
            aria-selected={isSelected}
            // Roving tabindex: only the cursor's option is in the tab order; the arrows move it.
            tabIndex={index === activeIndex ? 0 : -1}
            onClick={() => {
              setActiveName(pad.name);
              onSelect(pad.name);
            }}
            className={cn(
              "flex w-full flex-col gap-0.5 rounded-md py-1.5 pr-2.5 pl-2.5 text-left outline-none transition-colors duration-[var(--dur-select)] ease-out-quint",
              "focus-visible:ring-2 focus-visible:ring-sidebar-ring",
              isSelected
                ? "bg-[var(--sidebar-sel-fill)] hover:bg-[var(--sidebar-sel-fill-hover)]"
                : "hover:bg-sidebar-accent focus-visible:bg-sidebar-accent",
            )}
          >
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
        );
      })}
    </div>
  );
}
