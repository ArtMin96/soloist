import type { ReactNode } from "react";
import { DocumentList } from "@/components/orchestration/DocumentList";
import type { ScratchpadSummary } from "@/domain";

interface ScratchpadListProps {
  scratchpads: ScratchpadSummary[];
  selected: string | null;
  onSelect: (name: string) => void;
  /** The listbox's accessible name — lets a grouped roster label the active vs archived lists apart. */
  label?: string;
  /** Shown in place of the list when it is empty; defaults to the first-run guidance. */
  emptyHint?: ReactNode;
}

// The scratchpad instantiation of the shared document list (`DocumentList`): its rows read by the
// `data-scratchpad-name` handle attribute an e2e reader addresses, with the scratchpad first-run
// hint as the default empty state.
export function ScratchpadList({
  scratchpads,
  selected,
  onSelect,
  label = "Scratchpads",
  emptyHint,
}: ScratchpadListProps) {
  return (
    <DocumentList
      items={scratchpads}
      selected={selected}
      onSelect={onSelect}
      label={label}
      emptyHint={
        emptyHint ?? (
          <>
            No scratchpads yet. Agents create them to share a plan or research as they work — they
            will appear here live.
          </>
        )
      }
      handleAttribute={(name) => ({ "data-scratchpad-name": name })}
    />
  );
}
