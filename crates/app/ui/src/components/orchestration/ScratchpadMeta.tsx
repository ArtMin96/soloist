import { Clock3, Hash } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { formatUpdatedAt } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { ScratchpadSummary } from "@/domain";

/** The raw handle beside a humanized title. */
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
  variant?: "card" | "detail";
}

/** The revision token shared by scratchpad cards and detail headers. */
export function ScratchpadRevision({ revision }: { revision: number }) {
  return (
    <Badge
      {...{ [SCRATCHPAD_REVISION_ATTRIBUTE]: "" }}
      variant="muted"
      className="shrink-0 bg-toolbar-control font-mono tabular-nums text-toolbar-control-foreground"
    >
      Rev {revision}
    </Badge>
  );
}

/**
 * The scratchpad's address and recency as a compact, icon-led metadata rail.
 */
export function ScratchpadMeta({ pad, now, variant = "detail" }: ScratchpadMetaProps) {
  const updated = formatUpdatedAt(pad.updated_at, now);
  const handle = (
    <span
      {...{ [SCRATCHPAD_HANDLE_ATTRIBUTE]: "" }}
      title={`Handle: ${pad.name}`}
      className={cn("min-w-0 truncate font-mono", variant === "card" ? "type-label" : "type-body")}
    >
      {pad.name}
    </span>
  );
  const updatedAt = updated ? (
    <time
      {...{ [SCRATCHPAD_UPDATED_ATTRIBUTE]: "" }}
      dateTime={new Date(pad.updated_at).toISOString()}
    >
      {updated}
    </time>
  ) : (
    <span {...{ [SCRATCHPAD_UPDATED_ATTRIBUTE]: "" }}>Not recorded</span>
  );
  const archived = pad.archived ? (
    <Badge {...{ [SCRATCHPAD_ARCHIVED_ATTRIBUTE]: "" }} variant="muted" className="shrink-0">
      {ARCHIVED_LABEL}
    </Badge>
  ) : null;

  if (variant === "card") {
    return (
      <span className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 text-muted-foreground">
        <span className="flex min-w-0 items-center gap-1.5">
          <Hash aria-hidden className="size-3.5 shrink-0 text-accent" />
          <span className="sr-only">Handle </span>
          {handle}
        </span>
        <span className="type-label flex shrink-0 items-center gap-1.5">
          <Clock3 aria-hidden className="size-3.5 shrink-0" />
          <span className="sr-only">Updated </span>
          {updatedAt}
        </span>
        {archived}
      </span>
    );
  }

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-1.5 text-muted-foreground">
      <dl className="contents">
        <div className="flex min-w-0 items-center gap-1.5">
          <dt className="sr-only">Handle</dt>
          <Hash aria-hidden className="size-3.5 shrink-0 text-accent" />
          <dd className="min-w-0">{handle}</dd>
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <dt className="sr-only">Updated</dt>
          <Clock3 aria-hidden className="size-3.5 shrink-0" />
          <dd className="type-body">{updatedAt}</dd>
        </div>
      </dl>
      {archived}
    </div>
  );
}
