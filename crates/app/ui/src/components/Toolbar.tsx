import { Play, RotateCw, Square } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ToolbarProps {
  projectName: string;
  appVersion?: string;
  /** Whether a stack is loaded so bulk actions can run. */
  canBulk: boolean;
  onStartAll: () => void;
  onStopAll: () => void;
  onRestartRunning: () => void;
}

// The top bar: the loaded project and the stack-wide controls. Bulk actions route to the
// same core supervisor the per-row controls do — start/stop/restart implemented once.
export function Toolbar({
  projectName,
  appVersion,
  canBulk,
  onStartAll,
  onStopAll,
  onRestartRunning,
}: ToolbarProps) {
  return (
    <header className="flex h-11 shrink-0 items-center gap-2 border-b bg-sidebar px-3">
      <span className="text-[0.9375rem] font-[550] tracking-[-0.005em]">{projectName}</span>
      {appVersion && <span className="font-mono text-xs text-muted-foreground">v{appVersion}</span>}
      <div className="ml-auto flex items-center gap-1">
        <Button variant="ghost" size="sm" disabled={!canBulk} onClick={onStartAll}>
          <Play />
          Start all
        </Button>
        <Button variant="ghost" size="sm" disabled={!canBulk} onClick={onRestartRunning}>
          <RotateCw />
          Restart running
        </Button>
        <Button variant="ghost" size="sm" disabled={!canBulk} onClick={onStopAll}>
          <Square />
          Stop all
        </Button>
      </div>
    </header>
  );
}
