import { useCallback, useEffect, useRef, useState } from "react";
import { Pause, Play, X } from "lucide-react";
import { timerCancel, timerPause, timerResume } from "@/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  bodyPreview,
  fireBadge,
  formatCountdown,
  formatPausedRemaining,
  groupByOwner,
} from "@/store/timerPanel";
import { cn } from "@/lib/utils";
import type { AgentNode, TimerView } from "@/domain";

interface Props {
  timers: TimerView[];
  /** The flat agent list from the orchestration snapshot — for label lookups. */
  agents: AgentNode[];
  project: number;
}

// The timers panel: lists every armed and paused timer in the project, grouped by owning agent.
// Each timer shows its fire condition, waiting-on agents, a live countdown to the max-wait
// deadline, and a body preview. Pause/resume/cancel route to the core. An armed timer's countdown
// is computed client-side from `deadline_unix_millis` via requestAnimationFrame (no per-second
// backend events); a paused timer shows its frozen remaining time. Respects `prefers-reduced-motion`
// (numeric update only, no pulse).
export function TimersPanel({ timers, agents }: Props) {
  const labelOf = useCallback(
    (id: number) => agents.find((a) => a.id === id)?.label ?? `#${id}`,
    [agents],
  );

  if (timers.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-center">
        <p className="max-w-[28ch] text-[0.8125rem] text-muted-foreground">
          No active timers in this project.{" "}
          <span className="font-mono text-[0.75rem]">timer_set</span> and{" "}
          <span className="font-mono text-[0.75rem]">timer_fire_when_idle</span> arm them via MCP.
        </p>
      </div>
    );
  }

  const groups = groupByOwner(timers);

  return (
    <div className="flex flex-col divide-y overflow-auto">
      {Array.from(groups.entries()).map(([owner, ownerTimers]) => (
        <div key={owner}>
          {/* Agent header */}
          <div className="flex items-center gap-1.5 px-3 py-1.5 text-[0.6875rem] font-[550] text-muted-foreground">
            {labelOf(owner)}
          </div>
          {ownerTimers.map((timer) => (
            <TimerRow key={timer.id} timer={timer} labelOf={labelOf} />
          ))}
        </div>
      ))}
    </div>
  );
}

// ── Single timer row ────────────────────────────────────────────────────────────────────────────

interface RowProps {
  timer: TimerView;
  labelOf: (id: number) => string;
}

function TimerRow({ timer, labelOf }: RowProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const act = useCallback(async (fn: () => Promise<boolean>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  const isPaused = timer.status === "paused";
  const isIdleTimer = timer.fire.kind === "when_idle_any" || timer.fire.kind === "when_idle_all";

  return (
    <div
      className={cn(
        "group flex min-h-7 flex-col gap-0 border-t border-border/50 px-3 py-1.5 transition-colors",
        isPaused && "bg-sidebar",
      )}
    >
      {/* Main row */}
      <div className="flex items-center gap-2">
        {/* Fire-condition badge */}
        <Badge variant="outline" className={cn("shrink-0 tabular-nums", isPaused && "opacity-50")}>
          {isPaused ? "Paused" : fireBadge(timer.fire)}
        </Badge>

        {/* Waiting-on chips — shown only for idle timers with outstanding watches */}
        {isIdleTimer && timer.waiting_on.length > 0 && !isPaused && (
          <WaitingOnChips waiting={timer.waiting_on} labelOf={labelOf} />
        )}

        {/* Already-idle note */}
        {timer.already_idle && !isPaused && (
          <span className="shrink-0 text-[0.6875rem] text-muted-foreground italic">
            condition met
          </span>
        )}

        {/* Spacer */}
        <div className="flex-1" />

        {/* Countdown or paused-remaining */}
        <CountdownCell timer={timer} />

        {/* Ghost controls — always visible (no hidden-on-hover for a11y) */}
        <div className="flex shrink-0 items-center gap-0.5">
          {isPaused ? (
            <IconButton
              label="Resume timer"
              disabled={busy}
              onClick={() => act(() => timerResume(timer.owner, timer.id))}
            >
              <Play className="size-3" />
            </IconButton>
          ) : (
            <IconButton
              label="Pause timer"
              disabled={busy}
              onClick={() => act(() => timerPause(timer.owner, timer.id))}
            >
              <Pause className="size-3" />
            </IconButton>
          )}
          <IconButton
            label="Cancel timer"
            disabled={busy}
            onClick={() => act(() => timerCancel(timer.owner, timer.id))}
          >
            <X className="size-3" />
          </IconButton>
        </div>
      </div>

      {/* Body preview row */}
      <Tooltip>
        <TooltipTrigger asChild>
          <p className="cursor-default truncate text-[0.75rem] text-muted-foreground">
            {bodyPreview(timer.body)}
          </p>
        </TooltipTrigger>
        <TooltipContent
          side="bottom"
          className="max-w-[40ch] whitespace-pre-wrap font-mono text-[0.75rem]"
        >
          {timer.body}
        </TooltipContent>
      </Tooltip>

      {/* Inline error */}
      {error && (
        <p role="alert" className="text-[0.75rem] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

// ── Countdown cell ───────────────────────────────────────────────────────────────────────────────

function CountdownCell({ timer }: { timer: TimerView }) {
  const [display, setDisplay] = useState(() =>
    timer.status === "paused"
      ? formatPausedRemaining(timer.paused_remaining_millis ?? 0)
      : formatCountdown(timer.deadline_unix_millis - Date.now()),
  );
  const rafRef = useRef<number | null>(null);

  useEffect(() => {
    if (timer.status === "paused") {
      setDisplay(formatPausedRemaining(timer.paused_remaining_millis ?? 0));
      return;
    }

    const tick = () => {
      const remaining = timer.deadline_unix_millis - Date.now();
      setDisplay(formatCountdown(remaining));
      if (remaining > 0) {
        rafRef.current = requestAnimationFrame(tick);
      }
    };
    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current != null) cancelAnimationFrame(rafRef.current);
    };
  }, [timer.deadline_unix_millis, timer.status, timer.paused_remaining_millis]);

  return (
    <span
      className="shrink-0 font-mono text-[0.8125rem] tabular-nums text-foreground"
      aria-label={`Time remaining: ${display}`}
    >
      {display}
    </span>
  );
}

// ── Waiting-on chips ─────────────────────────────────────────────────────────────────────────────

const MAX_CHIPS = 3;

function WaitingOnChips({
  waiting,
  labelOf,
}: {
  waiting: number[];
  labelOf: (id: number) => string;
}) {
  const shown = waiting.slice(0, MAX_CHIPS);
  const overflow = waiting.length - MAX_CHIPS;

  return (
    <div
      className="flex min-w-0 shrink flex-wrap items-center gap-1"
      aria-label={`Waiting on ${waiting.length} agent${waiting.length === 1 ? "" : "s"}`}
    >
      {shown.map((id) => (
        <span
          key={id}
          className="rounded-full bg-muted px-1.5 py-0.5 text-[0.625rem] text-muted-foreground"
        >
          {labelOf(id)}
        </span>
      ))}
      {overflow > 0 && (
        <span className="text-[0.625rem] text-muted-foreground">+{overflow} more</span>
      )}
    </div>
  );
}

// ── Icon button ──────────────────────────────────────────────────────────────────────────────────

function IconButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="size-5 rounded p-0"
          aria-label={label}
          disabled={disabled}
          onClick={onClick}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="text-[0.75rem]">
        {label}
      </TooltipContent>
    </Tooltip>
  );
}
