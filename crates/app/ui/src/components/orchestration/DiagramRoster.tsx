import { useMemo, useState } from "react";
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
import { DiagramList } from "@/components/orchestration/DiagramList";
import {
  DIAGRAM_SORT_LABELS,
  DIAGRAM_SORT_ORDER,
  sortDiagrams,
  type DiagramSort,
} from "@/store/diagramSort";
import { humanizeName } from "@/lib/humanize";
import { cn } from "@/lib/utils";
import type { DiagramSummary } from "@/domain";

interface DiagramRosterProps {
  diagrams: DiagramSummary[];
  selected: string | null;
  onSelect: (name: string) => void;
}

// The diagram roster: a live search, an optional tag filter, and active/archived grouping over the
// keyboard-navigable list. It owns only view state (the query and the chosen tag) and derives the
// visible sets — the list rows, their roving-cursor behavior, and selection stay in `DiagramList`, so
// the roster's DOM contract (the "Diagrams" listbox) is unchanged. Archived documents get their own
// labelled list beneath the active ones, and only appear when some exist. Mirrors the scratchpad roster.
export function DiagramRoster({ diagrams, selected, onSelect }: DiagramRosterProps) {
  const [query, setQuery] = useState("");
  const [tag, setTag] = useState<string | null>(null);
  const [sort, setSort] = useState<DiagramSort>("updated");

  const tags = useMemo(() => {
    const distinct = new Set<string>();
    for (const diagram of diagrams) for (const each of diagram.tags) distinct.add(each);
    return [...distinct].sort();
  }, [diagrams]);

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matched = diagrams.filter((diagram) => {
      if (tag !== null && !diagram.tags.includes(tag)) return false;
      if (needle === "") return true;
      // Match both the handle and the title the row actually shows, so searching the prose a user
      // reads ("auth flow") finds a slug-named document just as its handle does.
      return (
        diagram.name.toLowerCase().includes(needle) ||
        humanizeName(diagram.name).toLowerCase().includes(needle) ||
        diagram.gist.toLowerCase().includes(needle)
      );
    });
    return sortDiagrams(matched, sort);
  }, [diagrams, query, tag, sort]);

  const active = visible.filter((diagram) => !diagram.archived);
  const archived = visible.filter((diagram) => diagram.archived);
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
              placeholder="Search diagrams…"
              aria-label="Search diagrams"
              className="h-7 pl-7 text-[0.8125rem]"
            />
          </div>
          <Select value={sort} onValueChange={(value) => setSort(value as DiagramSort)}>
            <SelectTrigger size="sm" aria-label="Sort diagrams" className="w-auto shrink-0 gap-1">
              <ArrowDownUp className="size-3.5 text-muted-foreground" aria-hidden />
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {DIAGRAM_SORT_ORDER.map((option) => (
                  <SelectItem key={option} value={option}>
                    {DIAGRAM_SORT_LABELS[option]}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>
        {tags.length > 0 && (
          <div className="flex flex-wrap gap-1" role="group" aria-label="Filter by tag">
            {tags.map((each) => {
              const isActive = tag === each;
              return (
                <button
                  key={each}
                  type="button"
                  aria-pressed={isActive}
                  onClick={() => setTag(isActive ? null : each)}
                  className={cn(
                    "rounded-md px-1.5 py-0.5 text-[0.6875rem] transition-colors duration-[var(--dur-fast)] ease-out-quint",
                    "focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none",
                    isActive
                      ? "bg-[var(--sidebar-sel-fill)] text-foreground"
                      : "text-muted-foreground hover:bg-sidebar-accent",
                  )}
                >
                  {each}
                </button>
              );
            })}
          </div>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        <DiagramList
          diagrams={active}
          selected={selected}
          onSelect={onSelect}
          emptyHint={
            filtering
              ? "No diagrams match your search."
              : "No diagrams yet. Agents create them to sketch an architecture or a flow as they work — they will appear here live."
          }
        />
        {archived.length > 0 && (
          <>
            <p className="type-label px-3 pt-3 pb-1 font-[550] text-muted-foreground">Archived</p>
            <DiagramList
              diagrams={archived}
              selected={selected}
              onSelect={onSelect}
              label="Archived diagrams"
            />
          </>
        )}
      </div>
    </div>
  );
}
