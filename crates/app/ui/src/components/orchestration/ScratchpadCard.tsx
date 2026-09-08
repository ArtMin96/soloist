import { CardRow } from "@/components/common/CardRow";
import { TagList } from "@/components/common/TagList";
import { ScratchpadMeta, ScratchpadRevision } from "@/components/orchestration/ScratchpadMeta";
import { humanizeName } from "@/lib/humanize";
import type { ScratchpadSummary } from "@/domain";

/** The title a row shows — the humanized name, not the handle. */
export const SCRATCHPAD_TITLE_ATTRIBUTE = "data-scratchpad-title";
/** The one-line summary of the body the core derives. */
export const SCRATCHPAD_GIST_ATTRIBUTE = "data-scratchpad-gist";

interface ScratchpadCardProps {
  pad: ScratchpadSummary;
  /** The clock reading the meta rail measures recency against. */
  now: number;
  /** Hands the pane over to this scratchpad's detail. */
  onOpen: () => void;
}

/**
 * One scratchpad in the board's list: its title beside the meta rail, the body's gist beneath, and a
 * wrapping tag rail. Nothing here is interactive — the whole block is the content of the row's own
 * button, so any control would nest inside another one.
 */
export function ScratchpadCard({ pad, now, onOpen }: ScratchpadCardProps) {
  return (
    <CardRow onOpen={onOpen}>
      <span className="flex w-full min-w-0 items-center gap-2">
        <span
          {...{ [SCRATCHPAD_TITLE_ATTRIBUTE]: "" }}
          className="type-body min-w-0 flex-1 truncate font-[550] text-foreground"
        >
          {humanizeName(pad.name)}
        </span>
        <ScratchpadRevision revision={pad.revision} />
      </span>

      <ScratchpadMeta pad={pad} now={now} variant="card" />

      {pad.gist && (
        <span
          {...{ [SCRATCHPAD_GIST_ATTRIBUTE]: "" }}
          className="type-label w-full min-w-0 truncate text-muted-foreground"
        >
          {pad.gist}
        </span>
      )}

      <TagList tags={pad.tags} wrap className="w-full" />
    </CardRow>
  );
}
