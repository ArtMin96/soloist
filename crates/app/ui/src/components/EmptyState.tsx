import { SquareTerminal } from "lucide-react";

// The main-pane resting state: no process is selected. Teaches the next action rather
// than just stating emptiness.
export function EmptyState({ hasProcesses }: { hasProcesses: boolean }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-background px-6 text-center">
      <SquareTerminal className="size-6 text-muted-foreground/50" aria-hidden />
      <p className="max-w-xs text-sm text-muted-foreground">
        {hasProcesses
          ? "Select a process in the sidebar to open its terminal."
          : "No processes yet. A solo.yml defines the stack to supervise."}
      </p>
    </div>
  );
}
