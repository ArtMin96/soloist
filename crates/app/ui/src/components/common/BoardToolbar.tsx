import type { ReactNode } from "react";
import { Search } from "lucide-react";
import { TagFilterChips } from "@/components/common/TagFilterChips";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";

/** The DOM handle the toolbar strip carries. */
export const BOARD_TOOLBAR_ATTRIBUTE = "data-board-toolbar";
/** The DOM handle the "{shown} of {total}" tally carries. */
export const BOARD_COUNT_ATTRIBUTE = "data-board-count";
/** The primary control that opens a board's create pane. */
export const BOARD_CREATE_ATTRIBUTE = "data-board-create";

export interface BoardToolbarProps {
  /** Plural noun the toolbar names its subject by: "todos" → placeholder "Search todos…", aria-label "Search todos". */
  subject: string;
  search: string;
  onSearchChange: (search: string) => void;
  /** Facet and arrangement controls set after the searchbox (a status select, a sort select, a view toggle). */
  facets?: ReactNode;
  /** How many rows survive the filter, and how many exist. */
  shown: number;
  total: number;
  /** The board's one filled action; omitted while a form is open. */
  primary?: ReactNode;
  tags: string[];
  tag: string | null;
  onTagChange: (tag: string | null) => void;
}

/**
 * A board's one header: search, the board's own facets, the result count, and the primary action,
 * wrapping within itself at narrow widths — never a second full-width strip. It owns no state — the
 * board holds the one filter and this renders and edits it — so the visible set and arrangement stay
 * pure functions of that state. The tag facet beneath appears only when tags exist.
 */
export function BoardToolbar({
  subject,
  search,
  onSearchChange,
  facets,
  shown,
  total,
  primary,
  tags,
  tag,
  onTagChange,
}: BoardToolbarProps) {
  return (
    <div
      {...{ [BOARD_TOOLBAR_ATTRIBUTE]: "" }}
      className="flex shrink-0 flex-col gap-1.5 border-b px-3 py-2"
    >
      <div className="flex flex-wrap items-center gap-2">
        {/* Sized to match the sm controls beside it; the leading icon's inset is the group's own,
            so nothing here hand-picks a padding to clear it. */}
        <InputGroup className="h-7 min-w-32 flex-1 basis-40">
          <InputGroupAddon className="text-icon-muted">
            <Search aria-hidden />
          </InputGroupAddon>
          <InputGroupInput
            type="search"
            value={search}
            onChange={(event) => onSearchChange(event.target.value)}
            placeholder={`Search ${subject}…`}
            aria-label={`Search ${subject}`}
          />
        </InputGroup>
        {facets}
        <output
          {...{ [BOARD_COUNT_ATTRIBUTE]: "" }}
          aria-live="polite"
          aria-atomic="true"
          className="type-label shrink-0 text-secondary-label"
        >
          {shown} of {total}
        </output>
        {primary}
      </div>
      <TagFilterChips tags={tags} active={tag} onToggle={onTagChange} />
    </div>
  );
}
