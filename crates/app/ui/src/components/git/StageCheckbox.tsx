import { Checkbox } from "@/components/ui/checkbox";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { stagedState, type StagedState } from "@/lib/git";
import type { GitFileStatus } from "@/domain";

/** What ticking the box does, per how much of the path is already recorded. */
const ACTION: Record<StagedState, string> = {
  staged: "Unstage",
  unstaged: "Stage",
  partial: "Stage the rest of",
};

/**
 * Whether a changed path is recorded for the next commit. Three states, because a path changed
 * on both sides of the index is part of each — ticking it then records the rest.
 *
 * Presentational: it reports the state it is handed and calls back. What staging means, and
 * whether this project may be changed at all, are the core's.
 */
export function StageCheckbox({
  path,
  status,
  disabled,
  onChange,
}: {
  path: string;
  status: GitFileStatus;
  disabled: boolean;
  onChange: (stage: boolean) => void;
}) {
  const state = stagedState(status);
  const label = `${ACTION[state]} ${path}`;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Checkbox
          aria-label={label}
          checked={state === "partial" ? "indeterminate" : state === "staged"}
          disabled={disabled}
          className="ms-1"
          onClick={(event) => event.stopPropagation()}
          onCheckedChange={(checked) => onChange(checked !== false)}
        />
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
