import type { ReactNode } from "react";
import { Play, RotateCw, ShieldCheck, Square } from "lucide-react";
import { Button } from "@/components/ui/button";
import { canRestart, canStart, canStop } from "@/lib/status";
import type { ProcStatus } from "@/domain";

type ControlSize = "icon-xs" | "icon-sm";

interface ProcessControlsProps {
  status: ProcStatus;
  onStart: () => void;
  onStop: () => void;
  onRestart: () => void;
  size?: ControlSize;
  /** A trust-gated command whose variant is not trusted — Start is blocked until then. */
  requiresTrust?: boolean;
  /** Trust the command; when provided and `requiresTrust`, a trust affordance is shown. */
  onTrust?: () => void;
}

// The per-process start / restart / stop cluster, reused in the sidebar row and the
// terminal header. Enabled state is derived from the status FSM (single source in
// `lib/status`), so a control is never offered for a transition the core would reject. An
// untrusted command blocks Start and surfaces a trust affordance (A6).
export function ProcessControls({
  status,
  onStart,
  onStop,
  onRestart,
  size = "icon-sm",
  requiresTrust = false,
  onTrust,
}: ProcessControlsProps) {
  return (
    <div className="flex items-center gap-0.5">
      {requiresTrust && onTrust && (
        <Control label="Trust" size={size} disabled={false} onClick={onTrust}>
          <ShieldCheck />
        </Control>
      )}
      <Control
        label="Start"
        size={size}
        disabled={!canStart(status) || requiresTrust}
        onClick={onStart}
      >
        <Play />
      </Control>
      <Control label="Restart" size={size} disabled={!canRestart(status)} onClick={onRestart}>
        <RotateCw />
      </Control>
      <Control label="Stop" size={size} disabled={!canStop(status)} onClick={onStop}>
        <Square />
      </Control>
    </div>
  );
}

function Control({
  label,
  size,
  disabled,
  onClick,
  children,
}: {
  label: string;
  size: ControlSize;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size={size}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={(event) => {
        // The row is itself clickable (select); a control click must not select it.
        event.stopPropagation();
        onClick();
      }}
    >
      {children}
    </Button>
  );
}
