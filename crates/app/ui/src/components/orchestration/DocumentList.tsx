import { useId, useState, type KeyboardEvent, type ReactNode } from "react";
import type { DocumentKind } from "@/components/orchestration/DocumentTitle";
import { humanizeName } from "@/lib/humanize";
import { Empty, EmptyDescription, EmptyHeader } from "@/components/ui/empty";
import { cn } from "@/lib/utils";

/** The row shape a document summary (scratchpad or diagram) must satisfy to appear in the list. */
export interface DocumentRow {
  id: number;
  name: string;
  revision: number;
  gist: string;
}

/** The DOM handle attribute each document kind's rows are stamped with — the single source an e2e
 *  reader (e.g. `ScratchpadPanel.ts`'s `NAME_ATTR`) and this list share, so the two can never drift
 *  apart and a reader addressing one document kind can never end up reading the other's. */
const DOCUMENT_NAME_ATTRIBUTE = {
  scratchpad: "data-scratchpad-name",
  diagram: "data-diagram-name",
} as const satisfies Record<DocumentKind, string>;

interface DocumentListProps<Row extends DocumentRow> {
  items: Row[];
  selected: string | null;
  onSelect: (name: string) => void;
  /** The listbox's accessible name — lets a grouped roster label the active vs archived lists apart. */
  label: string;
  /** Shown in place of the list when it is empty. */
  emptyHint: ReactNode;
  /** Which document kind these rows are — selects the row's name-handle attribute via
   *  `DOCUMENT_NAME_ATTRIBUTE`. */
  kind: DocumentKind;
}

// A single-select ARIA listbox shared by every document roster: one row per document (its humanized
// title over a one-line body gist with its revision in mono). The row still selects by the raw name
// handle — humanization is display only. Arrow keys / Home / End move the roving focus between
// options; Enter, Space, or a click opens the focused document. Activation is explicit (opening reads
// the full document) — scan with the arrows, commit with Enter. The option roles ride native
// <button>s so each is focusable and keyboard-operable, and the listbox rides a generic <div> so no
// list element's semantics are overridden. Presentational — selection and the choice arrive as props.
// The tint-in-place selection is the shared macOS source-list language, identical to the sidebar
// ProcessRow.
export function DocumentList<Row extends DocumentRow>({
  items,
  selected,
  onSelect,
  label,
  emptyHint,
  kind,
}: DocumentListProps<Row>) {
  const baseId = useId();
  // Track the roving cursor by the document's name, not its index, so a document added or removed
  // live keeps the cursor on the same document instead of sliding onto a neighbour.
  const [activeName, setActiveName] = useState<string | null>(selected);

  if (items.length === 0) {
    return (
      <Empty>
        <EmptyHeader>
          <EmptyDescription>{emptyHint}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  // Resolve the cursor to a live row; a name whose document was removed falls back to the first row.
  const activeIndex = Math.max(
    0,
    items.findIndex((item) => item.name === activeName),
  );
  const optionId = (index: number) => `${baseId}-option-${index}`;

  function moveTo(index: number) {
    const clamped = Math.max(0, Math.min(index, items.length - 1));
    setActiveName(items[clamped].name);
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
        moveTo(items.length - 1);
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
      // Marks the list as its own selection scope: its rows are azure-tinted only while the
      // keyboard is in here, and neutral once focus moves to the document (see index.css).
      data-selection-scope
      className="flex flex-col gap-px p-1 outline-none"
    >
      {items.map((item, index) => {
        const isSelected = item.name === selected;
        return (
          <button
            key={item.id}
            id={optionId(index)}
            type="button"
            role="option"
            aria-selected={isSelected}
            // The raw handle the row addresses, kept reachable now that the row reads as prose.
            {...{ [DOCUMENT_NAME_ATTRIBUTE[kind]]: item.name }}
            // Roving tabindex: only the cursor's option is in the tab order; the arrows move it.
            tabIndex={index === activeIndex ? 0 : -1}
            onClick={() => {
              setActiveName(item.name);
              onSelect(item.name);
            }}
            className={cn(
              // The source list's default row height, so a one-line row keeps the same rhythm as
              // the sidebar; a row carrying a gist grows to its second line from here.
              "flex min-h-7 w-full flex-col justify-center rounded-md px-2 py-1 text-left outline-none transition-colors duration-[var(--dur-select)] ease-out-quint",
              "focus-visible:ring-2 focus-visible:ring-sidebar-ring",
              isSelected
                ? "bg-[var(--sel-fill)] hover:bg-[var(--sel-fill-hover)]"
                : "hover:bg-sidebar-accent focus-visible:bg-sidebar-accent",
            )}
          >
            <span className="flex items-baseline gap-2">
              <span className="min-w-0 flex-1 truncate text-[0.8125rem] leading-4 text-foreground">
                {humanizeName(item.name)}
              </span>
              <span className="type-label shrink-0 font-mono tabular-nums text-muted-foreground">
                r{item.revision}
              </span>
            </span>
            {item.gist && (
              <span className="type-label truncate text-muted-foreground">{item.gist}</span>
            )}
          </button>
        );
      })}
    </div>
  );
}
