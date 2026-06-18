import type { ReactNode } from "react";
import { Play, RotateCw, Square } from "lucide-react";
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
}

// The per-process start / restart / stop cluster, reused in the sidebar row and the
// terminal header. Enabled state is derived from the status FSM (single source in
// `lib/status`), so a control is never offered for a transition the core would reject.
export function ProcessControls({
  status,
  onStart,
  onStop,
  onRestart,
  size = "icon-sm",
}: ProcessControlsProps) {
  return (
    <div className="flex items-center gap-0.5">
      <Control label="Start" size={size} disabled={!canStart(status)} onClick={onStart}>
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
