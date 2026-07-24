import type { DiagramSummary } from "@/domain";

// How the roster orders its diagrams. "updated" is the recency of the last source write; "name" is
// alphabetical — the same two orders the scratchpad roster offers.
export type DiagramSort = "updated" | "name";

// The order the sort options are offered in, and their compact labels. One source the roster's sort
// control renders.
export const DIAGRAM_SORT_ORDER: DiagramSort[] = ["updated", "name"];
export const DIAGRAM_SORT_LABELS: Record<DiagramSort, string> = {
  updated: "Recent",
  name: "Name",
};

// Orders diagrams for the roster without mutating the input. "updated" is most-recently-written first
// (ties — including documents that predate the timestamp and share updated_at 0 — fall back to name,
// so the order is stable); "name" is A–Z, case-insensitive.
export function sortDiagrams(diagrams: DiagramSummary[], sort: DiagramSort): DiagramSummary[] {
  const byName = (a: DiagramSummary, b: DiagramSummary) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  return [...diagrams].sort((a, b) =>
    sort === "name" ? byName(a, b) : b.updated_at - a.updated_at || byName(a, b),
  );
}
