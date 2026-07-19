import type { ScratchpadSummary } from "@/domain";

// How the roster orders its scratchpads. "updated" is the recency of the last body write; "name" is
// alphabetical — the two orders Solo's list offers.
export type ScratchpadSort = "updated" | "name";

// The order the sort options are offered in, and their compact labels. One source the roster's sort
// control renders.
export const SCRATCHPAD_SORT_ORDER: ScratchpadSort[] = ["updated", "name"];
export const SCRATCHPAD_SORT_LABELS: Record<ScratchpadSort, string> = {
  updated: "Recent",
  name: "Name",
};

// Orders scratchpads for the roster without mutating the input. "updated" is most-recently-written
// first (ties — including documents that predate the timestamp and share updated_at 0 — fall back to
// name, so the order is stable); "name" is A–Z, case-insensitive.
export function sortScratchpads(
  scratchpads: ScratchpadSummary[],
  sort: ScratchpadSort,
): ScratchpadSummary[] {
  const byName = (a: ScratchpadSummary, b: ScratchpadSummary) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  return [...scratchpads].sort((a, b) =>
    sort === "name" ? byName(a, b) : b.updated_at - a.updated_at || byName(a, b),
  );
}
