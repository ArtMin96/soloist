import type { ReactNode } from "react";
import { MinusIcon, PlusIcon, Undo2Icon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { HunkRange } from "@/domain";

const STAGE = "Stage this hunk";
const UNSTAGE = "Unstage this hunk";
const DISCARD = "Discard this hunk";

/**
 * What can be done with one hunk, beside the hunk itself.
 *
 * Which action is offered follows the comparison being read: the working side of a change can be
 * recorded or thrown away, the recorded side can only be taken back. The hunk is identified by
 * where it falls, never by its place in the rendered list, so these stay correct however the
 * viewer decides to mount and unmount rows.
 *
 * Presentational: it calls back with the hunk it was given.
 */
export function HunkActions({
  hunk,
  staged,
  busy,
  onStage,
  onUnstage,
  onDiscard,
}: {
  hunk: HunkRange;
  /** True when the hunk being read is what the index already holds. */
  staged: boolean;
  busy: boolean;
  onStage: (hunk: HunkRange) => void;
  onUnstage: (hunk: HunkRange) => void;
  onDiscard: (hunk: HunkRange) => void;
}) {
  return (
    <div className="flex items-center justify-end gap-1 border-b border-border bg-muted/50 px-2 py-1">
      {staged ? (
        <HunkButton
          label={UNSTAGE}
          icon={<MinusIcon />}
          busy={busy}
          onClick={() => onUnstage(hunk)}
        />
      ) : (
        <>
          <HunkButton label={STAGE} icon={<PlusIcon />} busy={busy} onClick={() => onStage(hunk)} />
          <HunkButton
            label={DISCARD}
            icon={<Undo2Icon />}
            busy={busy}
            onClick={() => onDiscard(hunk)}
          />
        </>
      )}
    </div>
  );
}

function HunkButton({
  label,
  icon,
  busy,
  onClick,
}: {
  label: string;
  icon: ReactNode;
  busy: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon-xs" aria-label={label} disabled={busy} onClick={onClick}>
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
