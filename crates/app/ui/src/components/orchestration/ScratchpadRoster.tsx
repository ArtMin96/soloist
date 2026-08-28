import { DocumentRoster, type DocumentRosterCopy } from "@/components/orchestration/DocumentRoster";
import {
  SCRATCHPAD_SORT_LABELS,
  SCRATCHPAD_SORT_ORDER,
  sortScratchpads,
} from "@/store/scratchpadSort";
import type { ScratchpadSummary } from "@/domain";

interface ScratchpadRosterProps {
  scratchpads: ScratchpadSummary[];
  selected: string | null;
  onSelect: (name: string) => void;
}

const COPY: DocumentRosterCopy = {
  label: "Scratchpads",
  archivedLabel: "Archived scratchpads",
  searchPlaceholder: "Search scratchpads…",
  searchAriaLabel: "Search scratchpads",
  sortAriaLabel: "Sort scratchpads",
  firstRunHint: (
    <>
      No scratchpads yet. Agents create them to share a plan or research as they work — they will
      appear here live.
    </>
  ),
  noResultsHint: "No scratchpads match your search.",
};

// The scratchpad instantiation of the shared document roster (`DocumentRoster`), sorted by
// `scratchpadSort` and read by the `data-scratchpad-name` handle attribute.
export function ScratchpadRoster({ scratchpads, selected, onSelect }: ScratchpadRosterProps) {
  return (
    <DocumentRoster
      items={scratchpads}
      selected={selected}
      onSelect={onSelect}
      copy={COPY}
      initialSort="updated"
      sortOrder={SCRATCHPAD_SORT_ORDER}
      sortLabels={SCRATCHPAD_SORT_LABELS}
      sortItems={sortScratchpads}
      kind="scratchpad"
    />
  );
}
