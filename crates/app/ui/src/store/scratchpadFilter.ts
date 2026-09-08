import {
  isSearchingOrTagging,
  matchesSearchAndTag,
  type SearchTagFilter,
} from "@/store/boardFilter";
import { humanizeName } from "@/lib/humanize";
import type { ScratchpadSummary } from "@/domain";

// The archived facet: every scratchpad, or one side of the listing flag. "all" is the unfiltered
// default, kept distinct from the flag so the board never compares against a bare boolean.
export type ArchivedFilter = "all" | "active" | "archived";

// The board's filter state: the shared needle and tag, plus the scratchpad board's own archived
// facet. Pure data — the summaries arrive from the live snapshot and this only narrows them, so the
// visible set stays trivially unit-testable and holds no IPC.
export interface ScratchpadFilter extends SearchTagFilter {
  archived: ArchivedFilter;
}

export const EMPTY_SCRATCHPAD_FILTER: ScratchpadFilter = { search: "", archived: "all", tag: null };

// The order the archived options are offered in, and their labels. One source the facet renders.
export const ARCHIVED_FILTER_ORDER: ArchivedFilter[] = ["all", "active", "archived"];
export const ARCHIVED_FILTER_LABELS: Record<ArchivedFilter, string> = {
  all: "All",
  active: "Active",
  archived: "Archived",
};

// Which scratchpads each option of the archived facet keeps. A record rather than a chain of
// comparisons, so an option added to the facet cannot compile until it says what it shows.
const ARCHIVED_FACET: Record<ArchivedFilter, (pad: ScratchpadSummary) => boolean> = {
  all: () => true,
  active: (pad) => !pad.archived,
  archived: (pad) => pad.archived,
};

/**
 * Narrows the scratchpads to those matching every active facet: the archived facet ANDed with the
 * shared needle and tag. The needle is matched against the handle, the humanized title the board
 * actually shows, and the gist — a reader searching for the words on screen must find the row
 * carrying them, whichever of the three they came from.
 */
export function filterScratchpads(
  pads: ScratchpadSummary[],
  filter: ScratchpadFilter,
): ScratchpadSummary[] {
  return pads.filter(
    (pad) =>
      ARCHIVED_FACET[filter.archived](pad) &&
      matchesSearchAndTag(pad, filter, (it) => [it.name, humanizeName(it.name), it.gist]),
  );
}

/** Whether any facet is narrowing the board — it picks its empty-state hint and whether to group
 *  its rows from this. */
export function isFilteringScratchpads(filter: ScratchpadFilter): boolean {
  return filter.archived !== EMPTY_SCRATCHPAD_FILTER.archived || isSearchingOrTagging(filter);
}
