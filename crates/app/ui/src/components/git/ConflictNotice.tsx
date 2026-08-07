import { TriangleAlertIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { FileChange } from "@/domain";

const CONFLICTED = "conflicted";
const ABANDON_LABEL = "Abandon merge";

/**
 * That a merge left work unresolved, and how much of it.
 *
 * Resolving is the reader's own job in their editor — Soloist shows the state and stays out of it.
 * The count is what makes the state actionable at a glance; the paths themselves are already in the
 * Changes tree, each carrying its own conflict letter, so repeating them here would be a second
 * list to keep in step with the first.
 *
 * Abandoning is offered only where there is a merge to abandon, which is a separate fact from there
 * being conflicts: putting stashed changes back can conflict too, and there is no merge behind that
 * one.
 */
export function ConflictNotice({
  changes,
  merging,
  busy,
  onAbandon,
}: {
  changes: FileChange[];
  /** Whether a merge is under way, which is what makes abandoning it possible. */
  merging: boolean;
  busy: boolean;
  onAbandon: () => void;
}) {
  const conflicted = changes.filter(
    (change) => change.status.unstaged === CONFLICTED || change.status.staged === CONFLICTED,
  ).length;
  if (conflicted === 0 && !merging) return null;

  return (
    <div
      role="status"
      className="flex shrink-0 items-center gap-2 border-t border-sidebar-border bg-git-conflicted/8 px-3 py-2"
    >
      <TriangleAlertIcon aria-hidden className="size-3.5 shrink-0 text-git-conflicted" />
      <p className="min-w-0 flex-1 text-[0.8125rem] text-git-conflicted">
        {conflicted > 0
          ? `${conflicted} ${conflicted === 1 ? "file needs" : "files need"} resolving`
          : "Merge in progress"}
      </p>
      {merging && (
        <Button variant="ghost" size="sm" disabled={busy} onClick={onAbandon}>
          {ABANDON_LABEL}
        </Button>
      )}
    </div>
  );
}
