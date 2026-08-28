import {
  CircleCheckIcon,
  CircleDashedIcon,
  CircleHelpIcon,
  CircleMinusIcon,
  CircleSlashIcon,
  CircleXIcon,
  ExternalLinkIcon,
  SendIcon,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { CHECK } from "@/lib/git";
import { Empty, EmptyDescription, EmptyHeader } from "@/components/ui/empty";
import { cn } from "@/lib/utils";
import type { CheckRun, CheckState } from "@/domain";

const HAND_OFF_LABEL = "Hand to an agent";
const OPEN_LABEL = "Open this check";
const NOTHING = "Nothing has reported a check on this branch yet.";

/** The glyph each state wears, so a check reads without its colour. */
const GLYPH: Record<CheckState, LucideIcon> = {
  pending: CircleDashedIcon,
  passed: CircleCheckIcon,
  failed: CircleXIcon,
  skipped: CircleMinusIcon,
  cancelled: CircleSlashIcon,
  unknown: CircleHelpIcon,
};

/**
 * What the service's checks say about the pull request, one row each.
 *
 * Presentational: props in, callbacks out. Which states exist, which of them a handoff is worth
 * offering for, and what a handoff says are the core's answers — this renders them.
 */
export function CheckList({
  checks,
  onHandOff,
  onOpen,
}: {
  checks: CheckRun[];
  /** Offered on the checks the core marks as objecting; absent on the rest. */
  onHandOff: (check: CheckRun) => void;
  onOpen: (url: string) => void;
}) {
  if (checks.length === 0) {
    return (
      <Empty className="p-4">
        <EmptyHeader>
          <EmptyDescription>{NOTHING}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }
  return (
    <ul className="flex flex-col">
      {checks.map((check) => (
        <CheckRow key={check.name} check={check} onHandOff={onHandOff} onOpen={onOpen} />
      ))}
    </ul>
  );
}

function CheckRow({
  check,
  onHandOff,
  onOpen,
}: {
  check: CheckRun;
  onHandOff: (check: CheckRun) => void;
  onOpen: (url: string) => void;
}) {
  const display = CHECK[check.state];
  const Glyph = GLYPH[check.state];
  return (
    <li className="group/check flex h-8 items-center gap-2 rounded-md px-1 hover:bg-muted">
      <Glyph
        aria-label={display.label}
        className={cn(
          "size-3.5 shrink-0",
          display.toneClass,
          // A check still running says so by moving, the same slow pulse a transitioning process
          // wears — and stops moving entirely for a reader who asked for that.
          check.state === "pending" && "motion-safe:animate-pulse",
        )}
      />
      <p className="min-w-0 flex-1 truncate text-[0.8125rem]">{check.name}</p>
      {check.workflow !== null && (
        <p className="hidden shrink-0 truncate text-[0.8125rem] text-muted-foreground sm:block">
          {check.workflow}
        </p>
      )}
      <span className={cn("shrink-0 type-label", display.toneClass)}>{display.label}</span>
      {check.state === "failed" && (
        <RowButton label={HAND_OFF_LABEL} onClick={() => onHandOff(check)}>
          <SendIcon />
        </RowButton>
      )}
      {check.url !== null && (
        <RowButton label={OPEN_LABEL} onClick={() => onOpen(check.url ?? "")}>
          <ExternalLinkIcon />
        </RowButton>
      )}
    </li>
  );
}

/** A quiet action on a row: present at rest for the keyboard, drawn only under the pointer. */
function RowButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={label}
          className="shrink-0 opacity-0 transition-opacity group-hover/check:opacity-100 focus-visible:opacity-100"
          onClick={onClick}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
