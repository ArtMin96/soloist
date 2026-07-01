import type { ReactNode } from "react";
import { History, Play, RotateCw, ShieldCheck, Square } from "lucide-react";
import { Button } from "@/components/ui/button";
import { processActions } from "@/lib/processActions";
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
  /** A stopped agent whose provider supports "Resume last session" — when set with
   *  `onResume`, a Resume control sits beside Start, enabled from the same resting states. */
  resumable?: boolean;
  /** Resume the agent's last session; shown only for a `resumable` process. */
  onResume?: () => void;
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
  resumable = false,
  onResume,
}: ProcessControlsProps) {
  // The same single source the palettes read decides which actions are live; the cluster shows the
  // full set of affordances and disables the ones that aren't currently runnable, so it never
  // reflows as a process changes state.
  const available = new Set(processActions({ status, requiresTrust, resumable }));
  return (
    <div className="flex items-center gap-0.5">
      {requiresTrust && onTrust && (
        <Control label="Trust" size={size} disabled={!available.has("trust")} onClick={onTrust}>
          <ShieldCheck />
        </Control>
      )}
      <Control label="Start" size={size} disabled={!available.has("start")} onClick={onStart}>
        <Play />
      </Control>
      {resumable && onResume && (
        <Control
          label="Resume last session"
          size={size}
          disabled={!available.has("resume")}
          onClick={onResume}
        >
          <History />
        </Control>
      )}
      <Control label="Restart" size={size} disabled={!available.has("restart")} onClick={onRestart}>
        <RotateCw />
      </Control>
      <Control label="Stop" size={size} disabled={!available.has("stop")} onClick={onStop}>
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
