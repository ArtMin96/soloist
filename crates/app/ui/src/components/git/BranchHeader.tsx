import { ArrowDownIcon, ArrowUpIcon, GitBranchIcon } from "lucide-react";
import { syncLabel } from "@/lib/git";
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

  return (
    <div className="flex h-9 min-w-0 items-center gap-1.5 border-b border-sidebar-border px-3">
      <GitBranchIcon aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="type-title min-w-0 truncate font-[550] tracking-[var(--tracking-title)]">
            {name}
          </span>
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
