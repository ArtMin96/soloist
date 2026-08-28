import { DocumentRoster, type DocumentRosterCopy } from "@/components/orchestration/DocumentRoster";
import { DIAGRAM_SORT_LABELS, DIAGRAM_SORT_ORDER, sortDiagrams } from "@/store/diagramSort";
import type { DiagramSummary } from "@/domain";

interface DiagramRosterProps {
  diagrams: DiagramSummary[];
  selected: string | null;
  onSelect: (name: string) => void;
}

const COPY: DocumentRosterCopy = {
  label: "Diagrams",
  archivedLabel: "Archived diagrams",
  searchPlaceholder: "Search diagrams…",
  searchAriaLabel: "Search diagrams",
  sortAriaLabel: "Sort diagrams",
  firstRunHint: (
    <>
      No diagrams yet. Agents create them to sketch an architecture or a flow as they work — they
      will appear here live.
    </>
  ),
  noResultsHint: "No diagrams match your search.",
};

// The diagram instantiation of the shared document roster (`DocumentRoster`), sorted by
// `diagramSort` and read by the `data-diagram-name` handle attribute.
export function DiagramRoster({ diagrams, selected, onSelect }: DiagramRosterProps) {
  return (
    <DocumentRoster
      items={diagrams}
      selected={selected}
      onSelect={onSelect}
      copy={COPY}
      initialSort="updated"
      sortOrder={DIAGRAM_SORT_ORDER}
      sortLabels={DIAGRAM_SORT_LABELS}
      sortItems={sortDiagrams}
      kind="diagram"
    />
  );
}
