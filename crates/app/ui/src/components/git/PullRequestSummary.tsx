import { ExternalLinkIcon, GitPullRequestArrowIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { PullRequest, PullRequestState } from "@/domain";

const OPEN_LABEL = "Open on the forge";

/** What each state is called, so no surface reads a wire value out loud. */
const STATE_LABEL: Record<PullRequestState, string> = {
  open: "Open",
  closed: "Closed",
  merged: "Merged",
};

/** The draft marker, which is a different fact from being open. */
const DRAFT_LABEL = "Draft";

/**
 * The pull request this branch already has: what it is, where it stands, and the one thing anybody
 * wants from it — a way to go and read it.
 *
 * Presentational: props in, callbacks out.
 */
export function PullRequestSummary({
  pullRequest,
  onOpen,
}: {
  pullRequest: PullRequest;
  onOpen: (url: string) => void;
}) {
  return (
    <div className="flex flex-col gap-2 rounded-md border border-border p-3">
      <div className="flex min-w-0 items-center gap-2">
        <GitPullRequestArrowIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        <span className="shrink-0 font-mono text-[0.8125rem] text-muted-foreground">
          {`#${pullRequest.number}`}
        </span>
        <p className="min-w-0 flex-1 truncate text-[0.8125rem] font-[550]">{pullRequest.title}</p>
        {pullRequest.draft && <StateBadge>{DRAFT_LABEL}</StateBadge>}
        <StateBadge>{STATE_LABEL[pullRequest.state]}</StateBadge>
      </div>
      <div className="flex items-center gap-2">
        <p className="min-w-0 flex-1 truncate font-mono text-[0.8125rem] text-muted-foreground">
          {`${pullRequest.head} → ${pullRequest.base}`}
        </p>
        <Button size="sm" variant="outline" onClick={() => onOpen(pullRequest.url)}>
          <ExternalLinkIcon aria-hidden />
          {OPEN_LABEL}
        </Button>
      </div>
    </div>
  );
}

/** A quiet monochrome marker: where a pull request stands is not process status, so it spends no
 *  saturated colour. */
function StateBadge({ children }: { children: string }) {
  return (
    <Badge variant="muted" className={cn("shrink-0 rounded-md px-2 type-label shadow-none")}>
      {children}
    </Badge>
  );
}
