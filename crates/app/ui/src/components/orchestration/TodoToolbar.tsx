import { Plus } from "lucide-react";
import { BoardToolbar } from "@/components/common/BoardToolbar";
import { Button } from "@/components/ui/button";
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

/** The status facet's unfiltered option, distinct from every declared `TodoStatus`. */
const ANY_STATUS: StatusFilter = "all";

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

// The to-do board's header: the shared board toolbar carrying this board's own two facets — the
// status select and the grouping toggle — and its one filled action. It owns no state: the board
// holds the single `TodoFilter` and `BoardView` and this renders and edits them, so the visible set
// and arrangement stay pure functions of that state.
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
    <BoardToolbar
      subject="todos"
      search={filter.search}
      onSearchChange={(search) => onChange({ ...filter, search })}
      shown={shown}
      total={total}
      tags={tags}
      tag={filter.tag}
      onTagChange={(tag) => onChange({ ...filter, tag })}
      facets={
        <>
          <Select
            value={filter.status}
            onValueChange={(value) => onChange({ ...filter, status: value as StatusFilter })}
          >
            <SelectTrigger size="sm" aria-label="Filter by status" className="w-32 shrink-0">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value={ANY_STATUS}>All statuses</SelectItem>
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
        </>
      }
      primary={
        onCreate && (
          <Button size="sm" onClick={onCreate} className="shrink-0">
            <Plus aria-hidden /> New todo
          </Button>
        )
      }
    />
  );
}
