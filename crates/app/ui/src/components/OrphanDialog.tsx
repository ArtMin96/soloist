import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { OrphanInfo } from "@/domain";

interface OrphanDialogProps {
  /** The surfaced leftover groups; `null` keeps the dialog closed. */
  orphans: OrphanInfo[] | null;
  onKillOne: (pgid: number) => void;
  onKillAll: () => void;
  onLeave: () => void;
}

// The reconciliation decision: leftover process groups from a previous run that match no
// known command. The user reaps them or leaves them running. Killing is destructive but
// stays slate (DESIGN.md spends saturated color only on status); the safe, reversible
// "Leave running" is the one azure primary and the Esc/backdrop default.
export function OrphanDialog({ orphans, onKillOne, onKillAll, onLeave }: OrphanDialogProps) {
  const open = orphans !== null && orphans.length > 0;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onLeave();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Leftover processes found</DialogTitle>
          <DialogDescription>
            These process groups were left running by a previous session and match no command in the
            current project. Reap them, or leave them running.
          </DialogDescription>
        </DialogHeader>

        <ul className="flex max-h-64 flex-col gap-1.5 overflow-y-auto">
          {orphans?.map((orphan) => (
            <li
              key={orphan.pgid}
              className="flex items-center gap-2 rounded-md border px-2.5 py-1.5"
            >
              <div className="min-w-0 flex-1">
                <p className="truncate text-[0.8125rem] font-medium">{orphan.name}</p>
                <p className="truncate font-mono text-xs text-muted-foreground">{orphan.command}</p>
              </div>
              <span className="shrink-0 font-mono text-xs text-muted-foreground/70">
                pgid {orphan.pgid}
              </span>
              <Button
                variant="ghost"
                size="xs"
                aria-label={`Kill ${orphan.name}`}
                onClick={() => onKillOne(orphan.pgid)}
              >
                Kill
              </Button>
            </li>
          ))}
        </ul>

        <DialogFooter>
          <Button variant="outline" onClick={onKillAll}>
            Kill all
          </Button>
          <Button onClick={onLeave}>Leave running</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
