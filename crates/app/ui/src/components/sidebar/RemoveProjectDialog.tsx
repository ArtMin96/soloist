import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface RemoveProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  projectName: string;
  /** How many of the project's processes are currently running (stopped by the removal). */
  runningCount: number;
  onConfirm: () => void;
}

// The project-removal confirmation: a genuine decision, so it earns a modal (DESIGN.md
// reserves dialogs for these). It states exactly what removal does in a grouped well —
// stops the running processes, forgets the project's Soloist state, keeps every file on
// disk — then offers one destructive action beside a ghost Cancel. There is no close X:
// the choice is the dialog's whole job, and the first focusable is Cancel, so Enter on
// arrival never destroys anything. Confirming closes immediately; the sidebar shows the
// teardown live as each process row leaves.
export function RemoveProjectDialog({
  open,
  onOpenChange,
  projectName,
  runningCount,
  onConfirm,
}: RemoveProjectDialogProps) {
  const rows: Array<[string, string]> = [];
  if (runningCount > 0) {
    rows.push([
      "Stops",
      runningCount === 1 ? "its 1 running process" : `its ${runningCount} running processes`,
    ]);
  }
  rows.push(["Forgets", "trust decisions, todos, scratchpads, and project settings"]);
  rows.push(["Keeps", "the project folder and solo.yml on disk, untouched"]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Remove “{projectName}”?</DialogTitle>
          <DialogDescription>
            Soloist stops managing this project. You can open the folder again later — it starts
            fresh, with commands untrusted.
          </DialogDescription>
        </DialogHeader>

        <dl className="divide-y divide-border rounded-lg border border-border text-xs">
          {rows.map(([label, value]) => (
            <div key={label} className="flex gap-2 px-3 py-2">
              <dt className="w-16 shrink-0 font-medium text-muted-foreground">{label}</dt>
              <dd className="min-w-0 flex-1 text-foreground/90">{value}</dd>
            </div>
          ))}
        </dl>

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="ghost">Cancel</Button>
          </DialogClose>
          <Button
            variant="destructive"
            onClick={() => {
              onConfirm();
              onOpenChange(false);
            }}
          >
            Remove project
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
