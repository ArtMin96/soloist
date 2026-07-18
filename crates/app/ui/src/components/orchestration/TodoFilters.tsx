import type { ReactNode } from "react";
import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TODO_STATUS } from "@/lib/todo";
import { cn } from "@/lib/utils";
import type { StatusFilter, TodoFilter } from "@/store/todoFilter";
import type { TodoStatus } from "@/domain";

// The status facet's options, "All" plus the closed set of declared statuses. Exhaustive over
// `TodoStatus`, kept in the same workflow order the editor offers.
const STATUS_OPTIONS: TodoStatus[] = ["open", "in_progress", "blocked", "done"];

interface TodoFiltersProps {
  filter: TodoFilter;
  tags: string[];
  onChange: (filter: TodoFilter) => void;
  /** A trailing control on the search row — the board's "New todo" action. */
  trailing?: ReactNode;
}

// The board's filter bar: a live search over title and body, a status facet, and a tag facet. It
// owns no state — the board holds the one `TodoFilter` and this renders and edits it — so the
// visible set stays a pure function of that filter. The tag facet appears only when tags exist.
export function TodoFilters({ filter, tags, onChange, trailing }: TodoFiltersProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <div className="relative min-w-0 flex-1">
          <Search
            className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <Input
            type="search"
            value={filter.search}
            onChange={(event) => onChange({ ...filter, search: event.target.value })}
            placeholder="Search todos…"
            aria-label="Search todos"
            className="h-7 pl-7 text-[0.8125rem]"
          />
        </div>
        <Select
          value={filter.status}
          onValueChange={(value) => onChange({ ...filter, status: value as StatusFilter })}
        >
          <SelectTrigger size="sm" aria-label="Filter by status" className="w-32 shrink-0">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All statuses</SelectItem>
            {STATUS_OPTIONS.map((status) => (
              <SelectItem key={status} value={status}>
                {TODO_STATUS[status]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {trailing}
      </div>
      {tags.length > 0 && (
        <div className="flex flex-wrap gap-1" role="group" aria-label="Filter by tag">
          {tags.map((tag) => {
            const active = filter.tag === tag;
            return (
              <button
                key={tag}
                type="button"
                aria-pressed={active}
                onClick={() => onChange({ ...filter, tag: active ? null : tag })}
                className={cn(
                  "rounded-md px-1.5 py-0.5 text-[0.6875rem] transition-colors duration-[var(--dur-fast)] ease-out-quint",
                  "focus-visible:ring-2 focus-visible:ring-sidebar-ring focus-visible:outline-none",
                  active
                    ? "bg-[var(--sidebar-sel-fill)] text-foreground"
                    : "text-muted-foreground hover:bg-sidebar-accent",
                )}
              >
                {tag}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
