/**
 * The half of a board's filter every board shares: a text needle and a single tag. The facets a
 * board adds are its own named fields (`status`, `archived`), never a generic one, so a comparison
 * site still reads as the thing it narrows by.
 */
export interface SearchTagFilter {
  search: string;
  tag: string | null;
}

/**
 * Whether a row survives the shared facets: a trimmed, case-insensitive needle matching any of
 * `fields`, ANDed with the single tag. A blank needle matches all; a null tag matches all.
 *
 * The searched fields are the board's to choose — a roster searches the handle, the title it
 * displays and its gist, while the to-do board searches title and body — so the needle's meaning
 * stays with the surface that shows the text.
 */
export function matchesSearchAndTag<T extends { tags: string[] }>(
  item: T,
  filter: SearchTagFilter,
  fields: (item: T) => string[],
): boolean {
  if (filter.tag !== null && !item.tags.includes(filter.tag)) return false;
  const needle = filter.search.trim().toLowerCase();
  if (needle === "") return true;
  return fields(item).some((field) => field.toLowerCase().includes(needle));
}

/** Whether either shared facet is narrowing the list — a board reads its empty-state hint, and
 *  whether to group or flatten, from this. */
export function isSearchingOrTagging(filter: SearchTagFilter): boolean {
  return filter.search.trim() !== "" || filter.tag !== null;
}

/** The distinct tags across the rows, sorted — the tag facet's options. */
export function distinctTags(items: readonly { tags: string[] }[]): string[] {
  const distinct = new Set<string>();
  for (const item of items) for (const tag of item.tags) distinct.add(tag);
  return [...distinct].sort();
}
