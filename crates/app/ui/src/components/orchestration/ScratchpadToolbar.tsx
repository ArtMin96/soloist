import { Plus } from "lucide-react";
import { BOARD_CREATE_ATTRIBUTE, BoardToolbar } from "@/components/common/BoardToolbar";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  ARCHIVED_FILTER_LABELS,
  ARCHIVED_FILTER_ORDER,
  type ArchivedFilter,
  type ScratchpadFilter,
} from "@/store/scratchpadFilter";
import {
  SCRATCHPAD_SORT_LABELS,
  SCRATCHPAD_SORT_ORDER,
  type ScratchpadSort,
} from "@/store/scratchpadSort";

/** The plural noun the shared toolbar names this board's search by. */
const SUBJECT = "scratchpads";

/** Both facets are short fixed words, so they share one width and the strip does not step. */
const FACET_WIDTH = "w-28 shrink-0";

interface ScratchpadToolbarProps {
  filter: ScratchpadFilter;
  tags: string[];
  onChange: (filter: ScratchpadFilter) => void;
  sort: ScratchpadSort;
  onSortChange: (sort: ScratchpadSort) => void;
  /** How many scratchpads survive the filter, and how many exist. */
  shown: number;
  total: number;
  /** Opens the board's create pane. */
  onCreate?: () => void;
}

// The scratchpad board's header: the shared board toolbar carrying this board's own two facets —
// the archived filter and the sort order — and its one filled action. It owns no state: the board
// holds the single `ScratchpadFilter` and `ScratchpadSort` and this renders and edits them, so the
// visible set and its order stay pure functions of that state.
export function ScratchpadToolbar({
  filter,
  tags,
  onChange,
  sort,
  onSortChange,
  shown,
  total,
  onCreate,
}: ScratchpadToolbarProps) {
  return (
    <BoardToolbar
      subject={SUBJECT}
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
            value={filter.archived}
            onValueChange={(value) => onChange({ ...filter, archived: value as ArchivedFilter })}
          >
            <SelectTrigger size="sm" aria-label="Filter by archived state" className={FACET_WIDTH}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {ARCHIVED_FILTER_ORDER.map((option) => (
                  <SelectItem key={option} value={option}>
                    {ARCHIVED_FILTER_LABELS[option]}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          <Select value={sort} onValueChange={(value) => onSortChange(value as ScratchpadSort)}>
            <SelectTrigger size="sm" aria-label="Sort scratchpads" className={FACET_WIDTH}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {SCRATCHPAD_SORT_ORDER.map((option) => (
                  <SelectItem key={option} value={option}>
                    {SCRATCHPAD_SORT_LABELS[option]}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </>
      }
      primary={
        onCreate && (
          <Button
            {...{ [BOARD_CREATE_ATTRIBUTE]: "scratchpad" }}
            size="sm"
            onClick={onCreate}
            className="shrink-0"
          >
            <Plus aria-hidden /> New scratchpad
          </Button>
        )
      }
    />
  );
}
