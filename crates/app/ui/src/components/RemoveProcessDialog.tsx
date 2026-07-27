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
import type { ProcessView } from "@/domain";

/**
 * What removing a live process does, stated plainly — the dialog's whole argument.
 *
 * `workers` is how many agents this one spawned that are still running. They are separate managed
 * processes in their own process groups, so removing their lead does **not** stop them: they keep
 * running and re-root as top-level rows. Saying so matters — a confirmation that implied they died
 * with the lead would be asking the user to agree to something that will not happen.
 */
function removalEffects(workers: number): Array<[string, string]> {
  const effects: Array<[string, string]> = [
    ["Stops", "this process and the child processes it started"],
  ];
  if (workers > 0) {
    effects.push([
      "Leaves",
      workers === 1
        ? "the 1 agent it spawned still running, on its own"
        : `the ${workers} agents it spawned still running, on their own`,
    ]);
  }
  effects.push(["Discards", "its output — the scrollback is not saved anywhere"]);
  effects.push(["Keeps", "every file it wrote in the project, untouched"]);
  return effects;
}

interface RemoveProcessDialogProps {
  /** The live process awaiting confirmation; `null` closes the dialog. */
  process: ProcessView | null;
  /** How many agents this one spawned are still running — they outlive it. */
  workers: number;
  onConfirm: () => void;
  onDismiss: () => void;
}

// Removing a *live* agent or terminal kills a running child, so it earns a modal — DESIGN.md
// reserves dialogs for genuine decisions, which is why a resting process is removed without one.
// The copy names what is lost rather than asking "are you sure": the process is running now, its
// output is only in memory, and the folder on disk is not involved. There is no close X — the
// choice is the dialog's whole job — and Cancel is the first focusable, so Enter on arrival never
// destroys anything.
export function RemoveProcessDialog({
  process,
  workers,
  onConfirm,
  onDismiss,
}: RemoveProcessDialogProps) {
  return (
    <Dialog open={process !== null} onOpenChange={(open) => !open && onDismiss()}>
      <DialogContent showCloseButton={false}>
        <DialogHeader>
          <DialogTitle>Remove “{process?.label}”?</DialogTitle>
          <DialogDescription>
            It is still running. Removing stops it and takes its row out of the sidebar.
          </DialogDescription>
        </DialogHeader>

        <dl className="divide-y divide-border rounded-lg border border-border text-xs">
          {removalEffects(workers).map(([label, value]) => (
            <div key={label} className="flex gap-2 px-3 py-2">
              <dt className="w-20 shrink-0 font-medium text-muted-foreground">{label}</dt>
              <dd className="min-w-0 flex-1 text-foreground/90">{value}</dd>
            </div>
          ))}
        </dl>

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="ghost">Cancel</Button>
          </DialogClose>
          <Button variant="destructive" onClick={onConfirm}>
            Remove
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
