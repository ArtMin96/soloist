import { FolderOpen, SquareTerminal } from "lucide-react";
import { Button } from "@/components/ui/button";

interface EmptyStateProps {
  /** Whether a stack is loaded — guides between "open a project" and "pick a process". */
  hasProcesses: boolean;
  onOpenProject: () => void;
  /** Plain-language feedback about the last open; takes precedence over the resting copy. */
  notice?: string | null;
}

// The main-pane resting state: no process is selected. Teaches the next action rather than
// just stating emptiness — a quiet glyph in an inset well, a short headline, and (with no
// project loaded) the single primary action, which carries the one azure accent. After an
// open, `notice` confirms what happened (a config was created, or the folder had no commands)
// so the action is never silent — it shows even once a stack has loaded, until the next open.
export function EmptyState({ hasProcesses, onOpenProject, notice }: EmptyStateProps) {
  const headline = notice ? null : hasProcesses ? "Nothing selected" : "No project open";
  const message =
    notice ??
    (hasProcesses
      ? "Select a process in the sidebar to open its terminal."
      : "Open a folder with a solo.yml to supervise its stack.");
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 bg-background px-6 text-center">
      <div
        aria-hidden
        className="grid size-12 place-items-center rounded-lg border border-border bg-card text-muted-foreground/70"
      >
        <SquareTerminal className="size-6" />
      </div>
      <div className="space-y-1">
        {headline && (
          <h2 className="text-[0.9375rem] font-[550] tracking-[-0.005em] text-foreground">
            {headline}
          </h2>
        )}
        <p className="mx-auto max-w-xs text-[0.8125rem] text-pretty text-muted-foreground">
          {message}
        </p>
      </div>
      {!hasProcesses && (
        <Button onClick={onOpenProject}>
          <FolderOpen />
          Open project
        </Button>
      )}
    </div>
  );
}
