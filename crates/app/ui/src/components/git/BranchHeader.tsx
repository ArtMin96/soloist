import {
  ArrowDownIcon,
  ArrowUpIcon,
  ChevronsUpDownIcon,
  GitBranchIcon,
  GitPullRequestIcon,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { syncLabel } from "@/lib/git";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { BranchMenu, type BranchActions } from "@/components/git/BranchMenu";
import { SyncActions } from "@/components/git/SyncActions";
import type { BranchInfo, Branches } from "@/domain";

/** What a detached head is called where a branch name would go. */
const DETACHED = "Detached";
const SWITCH_LABEL = "Switch branch";
const PULL_REQUEST_LABEL = "Pull request";
const PULL_REQUEST_HINT = "Open this branch's pull request, or propose one";

/**
 * What is checked out, how it stands against its upstream, and everything that can be done about
 * either.
 *
 * The branch name is the switcher: the one thing naming what is checked out is also the way to check
 * out something else, so there is no second control competing with it. The standing against the
 * upstream keeps its place beside the sync actions, because the two answer each other — the arrows
 * say what is owed, the buttons settle it.
 *
 * Presentational: props in, callbacks out. Whether a switch, a delete, or an exchange with the remote
 * is allowed at all is the core's answer, reported where it was asked for.
 */
export function BranchHeader({
  branch,
  branches,
  exchanging,
  busy,
  sync,
  branchActions,
  onDeleteBranch,
  onBranchesOpen,
  onOpenPullRequest,
}: {
  branch: BranchInfo;
  /** The branches to offer once the switcher is open, or null until that read lands. */
  branches: Branches | null;
  exchanging: boolean;
  busy: boolean;
  sync: { fetch: () => void; pull: () => void; push: () => void; stop: () => void } | null;
  branchActions: BranchActions | null;
  onDeleteBranch: (name: string) => void;
  /** The switcher opened or closed; the branch list is read only while it is open. */
  onBranchesOpen: (open: boolean) => void;
  /** Show the pull-request view, or null while nothing here may change the repository. */
  onOpenPullRequest: (() => void) | null;
}) {
  const name = branch.name ?? DETACHED;
  const standing = syncLabel(branch.sync);
  const ahead = "ahead" in branch.sync ? branch.sync.ahead : 0;
  const behind = "behind" in branch.sync ? branch.sync.behind : 0;
  // Green confirms a tracking branch is known to match its upstream. Violet deliberately marks
  // a branch that is local-only, detached, or still needs reconciliation with its upstream.
  const isSynced = branch.upstream !== null && branch.sync.state === "up_to_date";
  const badge = (
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
  );

  return (
    <div className="flex h-11 min-w-0 items-center gap-2 border-b border-sidebar-border px-3">
      {/* Until the project is trusted no branch can be switched, so the name reports and nothing
          more — an inert control would be a lie about what the rail can do. */}
      {branchActions === null ? (
        <Tooltip>
          <TooltipTrigger asChild>{badge}</TooltipTrigger>
          <TooltipContent>{upstreamLabel(name, branch.upstream)}</TooltipContent>
        </Tooltip>
      ) : (
        <Popover onOpenChange={onBranchesOpen}>
          <Tooltip>
            <TooltipTrigger asChild>
              <PopoverTrigger
                aria-label={SWITCH_LABEL}
                className="flex min-w-0 shrink items-center gap-1 rounded-md focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
              >
                {badge}
                <ChevronsUpDownIcon aria-hidden className="size-3 shrink-0 text-muted-foreground" />
              </PopoverTrigger>
            </TooltipTrigger>
            <TooltipContent>{upstreamLabel(name, branch.upstream)}</TooltipContent>
          </Tooltip>
          <PopoverContent align="start" className="w-auto p-0">
            <BranchMenu
              branches={branches}
              actions={branchActions}
              busy={busy}
              onDelete={onDeleteBranch}
            />
          </PopoverContent>
        </Popover>
      )}
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
      {onOpenPullRequest !== null && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon-xs"
              className={standing === null && sync === null ? "ms-auto" : undefined}
              aria-label={PULL_REQUEST_LABEL}
              onClick={onOpenPullRequest}
            >
              <GitPullRequestIcon />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{PULL_REQUEST_HINT}</TooltipContent>
        </Tooltip>
      )}
      {sync !== null && (
        <div className={standing === null && onOpenPullRequest === null ? "ms-auto" : undefined}>
          <SyncActions
            branch={branch}
            exchanging={exchanging}
            onFetch={sync.fetch}
            onPull={sync.pull}
            onPush={sync.push}
            onStop={sync.stop}
          />
        </div>
      )}
    </div>
  );
}

/** What the branch tracks, said in full where the name itself may be truncated. */
function upstreamLabel(name: string, upstream: string | null): string {
  return upstream !== null ? `${name} → ${upstream}` : `${name} (tracking nothing)`;
}
