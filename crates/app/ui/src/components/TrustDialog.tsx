import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { ConfigSync, TrustReviewCommand } from "@/domain";
import type { TrustReview } from "@/store/useTrust";

interface TrustDialogProps {
  /** The open review; `null` (or no pending commands) keeps the dialog closed. */
  review: TrustReview | null;
  onTrustCommand: (name: string) => void;
  onTrustAll: () => void;
  onDismiss: () => void;
}

// The trust review (A9): a project's `solo.yml` changed and these commands need trust
// before they can run. The user reviews exactly what each will run — command, working
// directory, environment — then trusts them. Trusting is the one azure primary ("Trust
// all"); per-command and the dismiss are ghost (DESIGN.md spends saturated color only on
// status). Start stays blocked for anything left untrusted (the core gate enforces it).
export function TrustDialog({ review, onTrustCommand, onTrustAll, onDismiss }: TrustDialogProps) {
  const open = review !== null && review.commands.length > 0;

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onDismiss();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Trust changed commands</DialogTitle>
          <DialogDescription>
            This project’s solo.yml changed. Review what each command runs, then trust it — an
            untrusted command cannot start.
          </DialogDescription>
        </DialogHeader>

        {review && <DiffSummary diff={review.diff} />}

        <ul className="max-h-72 divide-y divide-border overflow-x-hidden overflow-y-auto rounded-lg border border-border">
          {review?.commands.map((command) => (
            <CommandReview
              key={command.name}
              command={command}
              onTrust={() => onTrustCommand(command.name)}
            />
          ))}
        </ul>

        <DialogFooter>
          <Button variant="ghost" onClick={onDismiss}>
            Not now
          </Button>
          <Button onClick={onTrustAll}>Trust all</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// One command's reviewable detail: name + the action to trust it, over its command,
// working directory, and environment in mono (the Mono-Means-Data rule).
function CommandReview({ command, onTrust }: { command: TrustReviewCommand; onTrust: () => void }) {
  const env = Object.entries(command.env);
  return (
    <li className="flex flex-col gap-1 px-3 py-2.5">
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-[0.8125rem] font-medium">{command.name}</span>
        <Button variant="outline" size="xs" aria-label={`Trust ${command.name}`} onClick={onTrust}>
          Trust
        </Button>
      </div>
      <code className="truncate font-mono text-xs text-muted-foreground">{command.command}</code>
      {command.working_dir && (
        <code className="truncate font-mono text-xs text-muted-foreground">
          in {command.working_dir}
        </code>
      )}
      {env.length > 0 && (
        <code className="truncate font-mono text-xs text-muted-foreground">
          {env.map(([key, value]) => `${key}=${value}`).join("  ")}
        </code>
      )}
    </li>
  );
}

// A compact overview of the file change driving the review. Each line appears only when
// it carries names, so the dialog shows just what actually changed.
function DiffSummary({ diff }: { diff: ConfigSync }) {
  const rows: Array<[string, string]> = [];
  if (diff.added.length > 0) rows.push(["Added", diff.added.join(", ")]);
  if (diff.updated.length > 0) rows.push(["Changed", diff.updated.join(", ")]);
  if (diff.removed.length > 0) rows.push(["Removed", diff.removed.join(", ")]);
  if (diff.renamed.length > 0)
    rows.push([
      "Renamed",
      diff.renamed.map((rename) => `${rename.from} → ${rename.to}`).join(", "),
    ]);
  if (rows.length === 0) return null;

  return (
    <dl className="flex flex-col gap-1 text-xs">
      {rows.map(([label, value]) => (
        <div key={label} className="flex gap-2">
          <dt className="w-16 shrink-0 text-muted-foreground">{label}</dt>
          <dd className="min-w-0 flex-1 truncate font-mono text-muted-foreground">{value}</dd>
        </div>
      ))}
    </dl>
  );
}
