import { Badge } from "@/components/ui/badge";
import { formatUpdatedAt } from "@/lib/format";
import { distinctHandle } from "@/lib/humanize";
import type { ScratchpadSummary } from "@/domain";

/** The raw handle beside a humanized title, present only when the two read differently. */
export const SCRATCHPAD_HANDLE_ATTRIBUTE = "data-scratchpad-handle";
/** The revision the document currently sits at. */
export const SCRATCHPAD_REVISION_ATTRIBUTE = "data-scratchpad-revision";
/** How long ago the body was last written. */
export const SCRATCHPAD_UPDATED_ATTRIBUTE = "data-scratchpad-updated";
/** The listing flag that keeps an archived document out of the active roster. */
export const SCRATCHPAD_ARCHIVED_ATTRIBUTE = "data-scratchpad-archived";

/** What the archived chip says, so the card and the detail rail cannot word it differently. */
const ARCHIVED_LABEL = "Archived";

interface ScratchpadMetaProps {
  pad: ScratchpadSummary;
  /** The clock reading recency is measured against — passed in so the rail stays pure. */
  now: number;
}

/**
 * Everything a scratchpad says about itself that is not its title: its handle, its revision, when it
 * was last written and whether it is archived. A fragment rather than a box, because the card lays
 * these out on a truncating line and the detail header lays them out in a wrapping rail — the chips
 * are what must not differ between the two, not the spacing around them.
 */
export function ScratchpadMeta({ pad, now }: ScratchpadMetaProps) {
  const handle = distinctHandle(pad.name);
  const updated = formatUpdatedAt(pad.updated_at, now);

  return (
    <>
      {handle && (
        <span
          {...{ [SCRATCHPAD_HANDLE_ATTRIBUTE]: "" }}
          title={`Handle: ${handle}`}
          className="type-label min-w-0 shrink truncate font-mono text-muted-foreground"
        >
          {handle}
        </span>
      )}

      <span
        {...{ [SCRATCHPAD_REVISION_ATTRIBUTE]: "" }}
        className="type-label shrink-0 font-mono tabular-nums text-muted-foreground"
      >
        r{pad.revision}
      </span>

      {/* A document written before the core recorded write times has no stamp to show, and a
          fallback would date it to 1970 — so nothing is rendered rather than a wrong time. */}
      {updated && (
        <time
          {...{ [SCRATCHPAD_UPDATED_ATTRIBUTE]: "" }}
          dateTime={new Date(pad.updated_at).toISOString()}
          className="type-label shrink-0 text-muted-foreground"
        >
          {updated}
        </time>
      )}

      {pad.archived && (
        <Badge {...{ [SCRATCHPAD_ARCHIVED_ATTRIBUTE]: "" }} variant="muted" className="shrink-0">
          {ARCHIVED_LABEL}
        </Badge>
      )}
    </>
  );
}
