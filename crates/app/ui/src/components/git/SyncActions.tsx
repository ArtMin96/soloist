import { ArrowDownToLineIcon, ArrowUpFromLineIcon, RefreshCwIcon, XIcon } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import type { BranchInfo, GitCapabilities } from "@/domain";

const FETCH_LABEL = "Fetch";
const FETCH_HINT = "Bring the remote's commits in without touching the working tree";
const PULL_LABEL = "Pull";
const PULL_HINT = "Bring them in and reconcile them, however your git config says to";
const PUSH_LABEL = "Push";
const PUSH_HINT = "Hand this branch's commits to its upstream";
const PUBLISH_LABEL = "Publish";
const PUBLISH_HINT = "Hand this branch to the remote and track it from now on";
const STOP_LABEL = "Stop";
const STOP_HINT = "Stop waiting on the remote";

/**
 * Fetch, pull, and hand commits over — and, while one of those is under way, the one control that
 * ends it.
 *
 * A branch that tracks nothing is offered **Publish** rather than Push, because that is what handing
 * it over means for the first time; which one actually runs is the core's decision from the
 * repository's own state, so the two can never disagree. Stopping is not a fourth action but the
 * same three interrupted: reaching a remote is the only thing here that can outlast anybody's
 * patience, and a bounded wait with no way out reads as a frozen window.
 *
 * Presentational: props in, callbacks out.
 */
export function SyncActions({
  branch,
  capabilities,
  exchanging,
  onFetch,
  onPull,
  onPush,
  onStop,
}: {
  branch: BranchInfo;
  /** The remote actions the core can prove would advance this branch. */
  capabilities: Pick<GitCapabilities, "pull" | "push">;
  /** Whether an exchange is under way, which is when stopping is on offer instead. */
  exchanging: boolean;
  onFetch: () => void;
  onPull: () => void;
  onPush: () => void;
  onStop: () => void;
}) {
  const publishing = branch.upstream === null;

  if (exchanging) {
    return (
      <div className="flex shrink-0 items-center gap-1">
        <RefreshCwIcon
          aria-hidden
          className="size-3.5 text-muted-foreground motion-safe:animate-spin"
        />
        <IconButton label={STOP_LABEL} hint={STOP_HINT} icon={<XIcon />} onClick={onStop} />
      </div>
    );
  }

  return (
    <div className="flex shrink-0 items-center">
      <IconButton
        label={FETCH_LABEL}
        hint={FETCH_HINT}
        icon={<RefreshCwIcon />}
        onClick={onFetch}
      />
      {capabilities.pull && (
        <IconButton
          label={PULL_LABEL}
          hint={PULL_HINT}
          icon={<ArrowDownToLineIcon />}
          onClick={onPull}
        />
      )}
      {capabilities.push && (
        <IconButton
          label={publishing ? PUBLISH_LABEL : PUSH_LABEL}
          hint={publishing ? PUBLISH_HINT : PUSH_HINT}
          icon={<ArrowUpFromLineIcon />}
          onClick={onPush}
        />
      )}
    </div>
  );
}
