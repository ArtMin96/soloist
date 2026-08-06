import { ArrowDownIcon, ArrowUpIcon, GitBranchIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { syncLabel } from "@/lib/git";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { BranchInfo } from "@/domain";

/** What a detached head is called where a branch name would go. */
const DETACHED = "Detached";

/**
 * What is checked out and how it stands against its upstream. Reporting only — the branch
 * switcher and the sync actions arrive with the slice that can perform them, and an inert
 * control would be a lie about what the rail can do.
 */
export function BranchHeader({ branch }: { branch: BranchInfo }) {
  const name = branch.name ?? DETACHED;
  const standing = syncLabel(branch.sync);
  const ahead = "ahead" in branch.sync ? branch.sync.ahead : 0;
  const behind = "behind" in branch.sync ? branch.sync.behind : 0;
  // Green confirms a tracking branch is known to match its upstream. Violet deliberately marks
  // a branch that is local-only, detached, or still needs reconciliation with its upstream.
  const isSynced = branch.upstream !== null && branch.sync.state === "up_to_date";

  return (
    <div className="flex h-11 min-w-0 items-center gap-2 border-b border-sidebar-border px-3">
      <Tooltip>
        <TooltipTrigger asChild>
          <Badge
            variant="outline"
            className={cn(
              "h-6 min-w-0 max-w-full shrink gap-1.5 rounded-md px-2 text-[0.75rem] font-[550] tracking-[var(--tracking-body)] shadow-none",
              isSynced
                ? "border-git-branch-synced/40 bg-git-branch-synced/10 text-git-branch-synced"
                : "border-git-branch-local/40 bg-git-branch-local/10 text-git-branch-local",
            )}
          >
            <GitBranchIcon aria-hidden className="size-3.5 shrink-0" />
            <span className="min-w-0 truncate">{name}</span>
          </Badge>
        </TooltipTrigger>
        <TooltipContent>
          {branch.upstream ? `${name} → ${branch.upstream}` : `${name} (tracking nothing)`}
        </TooltipContent>
      </Tooltip>
      {standing !== null &&
        (ahead === 0 && behind === 0 ? (
          <span className="type-label ms-auto shrink-0 text-muted-foreground">{standing}</span>
        ) : (
          <span
            aria-label={standing}
            className="ms-auto flex shrink-0 items-center gap-1.5 font-mono text-[0.8125rem] text-muted-foreground"
          >
            {ahead > 0 && (
              <span aria-hidden className="flex items-center">
                <ArrowUpIcon className="size-3" />
                {ahead}
              </span>
            )}
            {behind > 0 && (
              <span aria-hidden className="flex items-center">
                <ArrowDownIcon className="size-3" />
                {behind}
              </span>
            )}
          </span>
        ))}
    </div>
  );
}
