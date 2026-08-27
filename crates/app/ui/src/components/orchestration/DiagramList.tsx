import type { ReactNode } from "react";
import { DocumentList } from "@/components/orchestration/DocumentList";
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

// The diagram instantiation of the shared document list (`DocumentList`): its rows read by the
// `data-diagram-name` handle attribute, with the diagram first-run hint as the default empty state.
export function DiagramList({
  diagrams,
  selected,
  onSelect,
  label = "Diagrams",
  emptyHint,
}: DiagramListProps) {
  return (
    <DocumentList
      items={diagrams}
      selected={selected}
      onSelect={onSelect}
      label={label}
      emptyHint={
        emptyHint ?? (
          <>
            No diagrams yet. Agents create them to sketch an architecture or a flow as they work —
            they will appear here live.
          </>
        )
      }
      handleAttribute={(name) => ({ "data-diagram-name": name })}
    />
  );
}
