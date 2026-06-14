import type { ProcStatus } from "@/domain";

// The single place a process status is turned into display text. The exhaustive
// Record makes the compiler require a label for every status, and `data-status`
// keeps styling (and tests) keyed off the value rather than scraped text.
const STATUS_LABELS: Record<ProcStatus, string> = {
  Stopped: "stopped",
  Starting: "starting",
  Running: "running",
  Crashed: "crashed",
  Restarting: "restarting",
  Stopping: "stopping",
  RestartExhausted: "restart-exhausted",
};

export function StatusBadge({ status }: { status: ProcStatus }) {
  return (
    <span data-status={status} className="text-muted-foreground">
      {STATUS_LABELS[status]}
    </span>
  );
}
