import type { ReactNode } from "react";
import { Play, RotateCw, Square } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ProjectControlsProps {
  onStartAll: () => void;
  onRestartRunning: () => void;
  onStopAll: () => void;
}

// The per-project bulk cluster in a project header: start every trusted auto-start
// command, restart the running ones, stop the stack. Each routes to the core supervisor
// scoped to this project — the same behaviour the per-row controls use, just project-wide.
// Ghost (slate) by DESIGN.md's one-accent rule; azure stays for the selected row.
export function ProjectControls({ onStartAll, onRestartRunning, onStopAll }: ProjectControlsProps) {
  return (
    <div className="flex items-center gap-0.5">
      <Bulk label="Start all" onClick={onStartAll}>
        <Play />
      </Bulk>
      <Bulk label="Restart running" onClick={onRestartRunning}>
        <RotateCw />
      </Bulk>
      <Bulk label="Stop all" onClick={onStopAll}>
        <Square />
      </Bulk>
    </div>
  );
}

function Bulk({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-label={label}
      title={label}
      onClick={(event) => {
        // The header toggles the project's collapse; a control click must not toggle it.
        event.stopPropagation();
        onClick();
      }}
    >
      {children}
    </Button>
  );
}
