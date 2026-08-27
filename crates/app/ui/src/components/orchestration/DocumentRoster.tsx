import { useMemo, useState, type ReactNode } from "react";
import { ArrowDownUp, Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { DocumentList, type DocumentRow } from "@/components/orchestration/DocumentList";
import type { DocumentKind } from "@/components/orchestration/DocumentTitle";
import { TagFilterChips } from "@/components/orchestration/TagFilterChips";
import { humanizeName } from "@/lib/humanize";

/** The row shape the roster's search, tag facet and archived grouping need on top of `DocumentRow`. */
export type DocumentSummaryRow = DocumentRow & { tags: string[]; archived: boolean };

/** The roster's per-subject display strings — the only thing that varies with what kind of document
 *  it lists. */
export interface DocumentRosterCopy {
  /** The active listbox's accessible name, e.g. "Scratchpads". */
  label: string;
  /** The archived listbox's accessible name, e.g. "Archived scratchpads". */
  archivedLabel: string;
  searchPlaceholder: string;
  searchAriaLabel: string;
  sortAriaLabel: string;
  /** Shown when the roster has no documents at all. */
  firstRunHint: ReactNode;
  /** Shown when a search or tag filter matches nothing. */
  noResultsHint: string;
}

interface DocumentRosterProps<Row extends DocumentSummaryRow, Sort extends string> {
  items: Row[];
  selected: string | null;
  onSelect: (name: string) => void;
  copy: DocumentRosterCopy;
  initialSort: Sort;
  sortOrder: Sort[];
  sortLabels: Record<Sort, string>;
  /** The subject's own sort — injected so the roster stays agnostic of which document kind it lists. */
  sortItems: (items: Row[], sort: Sort) => Row[];
  /** Which document kind these rows are — forwarded to `DocumentList` for its name-handle attribute. */
  kind: DocumentKind;
}

// The document roster shared by scratchpads and diagrams: a live search, an optional tag filter, and
// active/archived grouping over the keyboard-navigable list. It owns only view state (the query, the
// chosen tag and the chosen sort) and derives the visible sets — the list rows, their roving-cursor
// behavior, and selection stay in `DocumentList`, so the roster's DOM contract (its labelled listbox)
// is unchanged. Archived documents get their own labelled list beneath the active ones, and only
// appear when some exist.
export function DocumentRoster<Row extends DocumentSummaryRow, Sort extends string>({
  items,
  selected,
  onSelect,
  copy,
  initialSort,
  sortOrder,
  sortLabels,
  sortItems,
  kind,
}: DocumentRosterProps<Row, Sort>) {
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [sort, setSort] = useState<Sort>(initialSort);

  const tags = useMemo(() => {
    const distinct = new Set<string>();
    for (const item of items) for (const each of item.tags) distinct.add(each);
    return [...distinct].sort();
  }, [items]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matched = items.filter((item) => {
      if (tag !== null && !item.tags.includes(tag)) return false;
      if (needle === "") return true;
      // Match both the handle and the title the row actually shows, so searching the prose a user
      // reads ("editor design") finds a slug-named document just as its handle does.
      return (
        item.name.toLowerCase().includes(needle) ||
        humanizeName(item.name).toLowerCase().includes(needle) ||
        item.gist.toLowerCase().includes(needle)
      );
    });
    return sortItems(matched, sort);
  }, [items, query, tag, sort, sortItems]);

  const active = visible.filter((item) => !item.archived);
  const archived = visible.filter((item) => item.archived);
  const filtering = query.trim() !== "" || tag !== null;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-col gap-1.5 border-b p-2">
        <div className="flex items-center gap-2">
          <div className="relative min-w-0 flex-1">
            <Search
              className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden
            />
            <Input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={copy.searchPlaceholder}
              aria-label={copy.searchAriaLabel}
              className="h-7 pl-7 text-[0.8125rem]"
            />
          </div>
          <Select value={sort} onValueChange={(value) => setSort(value as Sort)}>
            <SelectTrigger
              size="sm"
              aria-label={copy.sortAriaLabel}
              className="w-auto shrink-0 gap-1"
            >
              <ArrowDownUp className="size-3.5 text-muted-foreground" aria-hidden />
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {sortOrder.map((option) => (
                  <SelectItem key={option} value={option}>
                    {sortLabels[option]}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>
        <TagFilterChips tags={tags} active={tag} onToggle={setTag} />
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        <DocumentList
          items={active}
          selected={selected}
          onSelect={onSelect}
          label={copy.label}
          emptyHint={filtering ? copy.noResultsHint : copy.firstRunHint}
          kind={kind}
        />
        {archived.length > 0 && (
          <>
            <p className="type-label px-3 pt-3 pb-1 font-[550] text-muted-foreground">Archived</p>
            <DocumentList
              items={archived}
              selected={selected}
              onSelect={onSelect}
              label={copy.archivedLabel}
              // Never rendered: this list only mounts once `archived.length > 0`, so its own empty
              // state can't occur.
              emptyHint={null}
              kind={kind}
            />
          </>
        )}
      </div>
    </div>
  );
}
