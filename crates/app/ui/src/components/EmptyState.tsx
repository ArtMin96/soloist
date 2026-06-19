import { FolderOpen, SquareTerminal } from "lucide-react";
import { Button } from "@/components/ui/button";

interface EmptyStateProps {
  /** Whether a stack is loaded — guides between "open a project" and "pick a process". */
  hasProcesses: boolean;
  onOpenProject: () => void;
}

// The main-pane resting state: no process is selected. Teaches the next action rather
// than just stating emptiness — with no project loaded, opening one is the single primary
// action, so it carries the one azure accent.
export function EmptyState({ hasProcesses, onOpenProject }: EmptyStateProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-background px-6 text-center">
      <SquareTerminal className="size-6 text-muted-foreground/50" aria-hidden />
      <p className="max-w-xs text-sm text-muted-foreground">
        {hasProcesses
          ? "Select a process in the sidebar to open its terminal."
          : "No project loaded. Open a folder with a solo.yml to supervise its stack."}
      </p>
      {!hasProcesses && (
        <Button onClick={onOpenProject}>
          <FolderOpen />
          Open project
        </Button>
      )}
    </div>
  );
}
