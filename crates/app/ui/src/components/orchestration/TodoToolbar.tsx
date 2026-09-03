import { Plus, Search } from "lucide-react";
import { TagFilterChips } from "@/components/orchestration/TagFilterChips";
import { Button } from "@/components/ui/button";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { BOARD_VIEWS, TODO_STATUS, TODO_STATUS_ORDER, type BoardView } from "@/lib/todo";
import type { StatusFilter, TodoFilter } from "@/store/todoFilter";

interface TodoToolbarProps {
  filter: TodoFilter;
  tags: string[];
  onChange: (filter: TodoFilter) => void;
  view: BoardView;
  onViewChange: (view: BoardView) => void;
  /** How many todos survive the filter, and how many exist. */
  shown: number;
  total: number;
  /** The board's primary action; omitted while the create form is open. */
  onCreate?: () => void;
}

// The board's one header: search, the status facet, the grouping toggle, the result count, and
// the primary action, wrapping within itself at narrow widths — never a second full-width strip.
// It owns no state — the board holds the one `TodoFilter` and `BoardView` and this renders and
// edits them — so the visible set and arrangement stay pure functions of that state. The tag facet
// beneath appears only when tags exist.
export function TodoToolbar({
  filter,
  tags,
  onChange,
  view,
  onViewChange,
  shown,
  total,
  onCreate,
}: TodoToolbarProps) {
  return (
    <div data-todo-toolbar className="flex shrink-0 flex-col gap-1.5 border-b px-3 py-2">
      <div className="flex flex-wrap items-center gap-2">
        {/* Sized to match the sm controls beside it; the leading icon's inset is the group's own,
            so nothing here hand-picks a padding to clear it. */}
        <InputGroup className="h-7 min-w-32 flex-1 basis-40">
          <InputGroupAddon>
            <Search aria-hidden />
          </InputGroupAddon>
          <InputGroupInput
            type="search"
            value={filter.search}
            onChange={(event) => onChange({ ...filter, search: event.target.value })}
            placeholder="Search todos…"
            aria-label="Search todos"
          />
        </InputGroup>
        <Select
          value={filter.status}
          onValueChange={(value) => onChange({ ...filter, status: value as StatusFilter })}
        >
          <SelectTrigger size="sm" aria-label="Filter by status" className="w-32 shrink-0">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectItem value="all">All statuses</SelectItem>
              {TODO_STATUS_ORDER.map((status) => (
                <SelectItem key={status} value={status}>
                  {TODO_STATUS[status]}
                </SelectItem>
              ))}
            </SelectGroup>
          </SelectContent>
        </Select>
        <ToggleGroup
          type="single"
          size="sm"
          value={view}
          onValueChange={(next) => {
            if (next) onViewChange(next as BoardView);
          }}
          aria-label="Group todos"
          className="shrink-0"
        >
          {BOARD_VIEWS.map((option) => (
            <ToggleGroupItem key={option.value} value={option.value}>
              {option.label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
        <span data-todo-count className="type-label shrink-0 text-muted-foreground">
          {shown} of {total}
        </span>
        {onCreate && (
          <Button size="sm" onClick={onCreate} className="shrink-0">
            <Plus aria-hidden /> New todo
          </Button>
        )}
      </div>
      <TagFilterChips
        tags={tags}
        active={filter.tag}
        onToggle={(tag) => onChange({ ...filter, tag })}
      />
    </div>
  );
}
