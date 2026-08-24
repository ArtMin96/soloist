import { useEffect, useState } from "react";
import { ArrowDownIcon, ArrowUpIcon, GitBranchIcon, GitPullRequestIcon } from "lucide-react";
import { BranchMenu } from "@/components/git/BranchMenu";
import { ConfirmDialog } from "@/components/git/ConfirmDialog";
import { SyncActions } from "@/components/git/SyncActions";
import { IconButton } from "@/components/IconButton";
import { Badge } from "@/components/ui/badge";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { branchStanding } from "@/lib/git";
import { cn } from "@/lib/utils";
import { onBranchSwitcherRequest, useBranchCluster } from "@/store/git/branchCluster";
import type { GitLineCounts } from "@/domain";

/** What a detached head is called where a branch name would go. */
const DETACHED = "Detached";
const SWITCH_LABEL = "Switch branch";
const PULL_REQUEST_LABEL = "Pull request";
const PULL_REQUEST_HINT = "Open this branch's pull request, or propose one";

/** The question asked before a branch is destroyed. */
const DELETE_BRANCH_TITLE = "Delete this branch?";
const DELETE_BRANCH_CONFIRM = "Delete";
const DELETE_BRANCH_CANCEL = "Keep it";

/**
 * What is checked out, how it stands against its upstream, and everything that can be done about
 * either — in the window chrome, beside the rest of the app's contextual controls.
 *
 * It lives here rather than in the rail because the rail is 280px wide and this cluster needs most
 * of that on its own: given the name, the standing, and four controls to fit, the branch name was
 * the only thing left that could shrink, and it shrank to nothing. The trailing end of the title bar
 * has the room, and puts what is checked out where a window says what it is looking at.
 *
 * Absent, not empty, when the project is not a repository — the same rule the attention control
 * follows, so nothing in the chrome is a standing claim about a project that has none to make.
 *
 * Presentational: it holds the name being typed into the switcher and the branch awaiting
 * confirmation, and nothing else. Whether a switch, a delete, or an exchange with the remote is
 * allowed at all is the core's answer, reported where it was asked for.
 */
export function BranchCluster() {
  const view = useBranchCluster();
  const [deleting, setDeleting] = useState<string | null>(null);
  // Held rather than left to the popover, so the command palette can reach this switcher instead of
  // carrying a second way to switch branches.
  const [switcherOpen, setSwitcherOpen] = useState(false);
  const { onBranchesOpen } = view ?? {};

  useEffect(() => onBranchSwitcherRequest(() => setSwitcherOpen(true)), []);
  // Whoever opened it, the surface that reads the branches is told from here — so a request from the
  // palette and a press on the badge take the same path rather than one of them reading nothing.
  useEffect(() => {
    onBranchesOpen?.(switcherOpen);
  }, [onBranchesOpen, switcherOpen]);

  if (view === null) return null;

  const { branch, branchActions, capabilities, lineCounts, exchange, openPullRequest } = view;
  const name = branch.name ?? DETACHED;
  const standing = branchStanding(branch);
  const hasMeasuredLines = lineCounts.additions > 0 || lineCounts.deletions > 0;
  const countsLabel = lineCountLabel(lineCounts);
  const badge = (
    <Badge variant="tinted" className={cn("min-w-0 shrink", standing.toneClass)}>
      <GitBranchIcon aria-hidden />
      <span className="type-body min-w-0 truncate font-[550] tracking-[var(--tracking-body)]">
        {name}
      </span>
    </Badge>
  );
  const counts =
    countsLabel === null ? null : (
      <span
        role="img"
        aria-label={countsLabel}
        tabIndex={lineCounts.complete ? undefined : 0}
        className={cn(
          "type-body flex shrink-0 items-center gap-1 rounded-sm font-mono tabular-nums",
          !hasMeasuredLines && "text-muted-foreground",
          !lineCounts.complete &&
            "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
        )}
      >
        {!hasMeasuredLines ? (
          <span aria-hidden>Lines —</span>
        ) : (
          <>
            {lineCounts.additions > 0 && (
              <span aria-hidden className="text-git-added">
                {!lineCounts.complete && "≥"}+{lineCounts.additions}
              </span>
            )}
            {lineCounts.deletions > 0 && (
              <span aria-hidden className="text-git-deleted">
                {!lineCounts.complete && "≥"}&minus;{lineCounts.deletions}
              </span>
            )}
          </>
        )}
      </span>
    );

  return (
    <div className="flex min-w-0 items-center gap-2">
      {/* Until the project is trusted no branch can be switched, so the name reports and nothing
          more — an inert control would be a lie about what the chrome can do. */}
      {branchActions === null ? (
        <Tooltip>
          <TooltipTrigger asChild>{badge}</TooltipTrigger>
          <TooltipContent>{upstreamLabel(name, branch.upstream)}</TooltipContent>
        </Tooltip>
      ) : (
        <Popover open={switcherOpen} onOpenChange={setSwitcherOpen}>
          <Tooltip>
            <TooltipTrigger asChild>
              <PopoverTrigger
                aria-label={SWITCH_LABEL}
                className="flex min-w-0 shrink items-center rounded-md focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
              >
                {badge}
              </PopoverTrigger>
            </TooltipTrigger>
            <TooltipContent>{upstreamLabel(name, branch.upstream)}</TooltipContent>
          </Tooltip>
          <PopoverContent align="end" className="w-auto p-0">
            <BranchMenu
              branches={view.branches}
              actions={branchActions}
              busy={view.busy}
              onDelete={setDeleting}
            />
          </PopoverContent>
        </Popover>
      )}
      {counts !== null &&
        (lineCounts.complete ? (
          counts
        ) : (
          <Tooltip>
            <TooltipTrigger asChild>{counts}</TooltipTrigger>
            <TooltipContent>{countsLabel}</TooltipContent>
          </Tooltip>
        ))}
      {standing.label !== null &&
        (standing.ahead === 0 && standing.behind === 0 ? (
          <span className="type-label shrink-0 text-muted-foreground">{standing.label}</span>
        ) : (
          // The arrows and their counts are one image of the standing, named by the words the label
          // carries: `role="img"` is what lets the name reach a reader who is not looking at them,
          // since a bare span is a generic element and ARIA forbids naming one.
          <span
            role="img"
            aria-label={standing.label}
            className="type-body flex shrink-0 items-center gap-1.5 font-mono tabular-nums text-muted-foreground"
          >
            {standing.ahead > 0 && (
              <span aria-hidden className="flex items-center">
                <ArrowUpIcon className="size-3" />
                {standing.ahead}
              </span>
            )}
            {standing.behind > 0 && (
              <span aria-hidden className="flex items-center">
                <ArrowDownIcon className="size-3" />
                {standing.behind}
              </span>
            )}
          </span>
        ))}
      {openPullRequest !== null && (
        <IconButton
          label={PULL_REQUEST_LABEL}
          hint={PULL_REQUEST_HINT}
          icon={<GitPullRequestIcon />}
          onClick={openPullRequest}
        />
      )}
      {exchange !== null && (
        <SyncActions
          branch={branch}
          capabilities={capabilities}
          exchanging={view.exchanging}
          onFetch={exchange.fetch}
          onPull={exchange.pull}
          onPush={exchange.push}
          onStop={exchange.stop}
        />
      )}
      <ConfirmDialog
        open={deleting !== null}
        title={DELETE_BRANCH_TITLE}
        confirm={DELETE_BRANCH_CONFIRM}
        cancel={DELETE_BRANCH_CANCEL}
        onCancel={() => setDeleting(null)}
        onConfirm={() => {
          if (deleting !== null) branchActions?.remove(deleting);
          setDeleting(null);
        }}
      >
        <span className="font-mono">{deleting}</span> will be removed. Version control refuses while
        it holds commits no other branch holds, and that refusal stands — there is no forced delete
        here.
      </ConfirmDialog>
    </div>
  );
}

/** What the branch tracks, said in full where the name itself may be truncated. */
function upstreamLabel(name: string, upstream: string | null): string {
  return upstream !== null ? `${name} → ${upstream}` : `${name} (tracking nothing)`;
}

function lineCountLabel(counts: GitLineCounts): string | null {
  if (counts.complete) {
    if (counts.additions === 0 && counts.deletions === 0) return null;
    return `${lineTotal(counts.additions, "addition")}, ${lineTotal(counts.deletions, "deletion")}`;
  }

  const measured = [
    counts.additions > 0 ? lineTotal(counts.additions, "addition") : null,
    counts.deletions > 0 ? lineTotal(counts.deletions, "deletion") : null,
  ].filter((total): total is string => total !== null);
  if (measured.length === 0) return "Line totals unavailable; measurement is incomplete";
  return `At least ${measured.join(" and ")}; line totals are incomplete`;
}

function lineTotal(total: number, kind: "addition" | "deletion"): string {
  return `${total} line ${total === 1 ? kind : `${kind}s`}`;
}
