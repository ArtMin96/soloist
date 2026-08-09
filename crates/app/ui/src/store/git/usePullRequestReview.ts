import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { gitHandOff, gitMergePullRequest, gitPullRequestReview } from "@/api";
import { useGitActions } from "@/store/git/useGitActions";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";
import type { Handoff, HandoffSubject, MergeMethod, PullRequestReview } from "@/domain";

/** The key the one project-wide merge is tracked under. */
const MERGE = "merge";

/**
 * How often an open review re-reads itself. A check finishing happens on somebody else's machine,
 * so there is no event to wait for — but the cost is a request to a service with a rate limit, so
 * the interval is deliberately slow and the refresh control is always there for somebody who is
 * watching a build.
 */
export const REVIEW_POLL_MS = 20_000;

/**
 * How long it waits instead while the last read failed. A service that is refusing — a rate limit,
 * a network that is down, an account that expired — must not be asked at the rate of one that is
 * answering.
 */
export const REVIEW_BACKOFF_MS = 120_000;

export interface PullRequestReviewStore {
  /** What the branch has open, or null until the first read lands and when it has nothing open. */
  review: PullRequestReview | null;
  /** True until that first read resolves. */
  loading: boolean;
  /** What the last refused action or failed read said. */
  error: string | null;
  /** Whether a merge is still being made. */
  merging: boolean;
  /** Re-reads now, whatever the poll was going to do. */
  refresh: () => void;
  /** Resolves true when the merge was carried out. */
  merge: (number: number, method: MergeMethod) => Promise<boolean>;
  /** Resolves what became of the handoff, or null when it was refused. */
  handOff: (subject: HandoffSubject, target: number | null) => Promise<Handoff | null>;
}

/**
 * What an open pull request looks like, kept current while the panel is open.
 *
 * Holds no rules of its own: what a check state means, which agent a handoff reaches, and whether a
 * merge is allowed are every one of them the core's. What lives here is the cadence — how often to
 * ask a service that announces nothing, and how to stop asking one that is refusing.
 *
 * The polling is bounded three ways. It runs only while this is mounted, so closing the panel and
 * quitting the app both end it; a request is never started while one is in flight, so a slow
 * service cannot pile them up; and a failed read slows the next attempt to
 * {@link REVIEW_BACKOFF_MS} rather than keeping the rate up against something that is refusing.
 */
export function usePullRequestReview(project: number | null): PullRequestReviewStore {
  const { busy, error: actionError, run } = useGitActions();
  const [handoffError, setHandoffError] = useState<string | null>(null);
  const inFlight = useRef(false);

  const read = useCallback(() => {
    if (project == null) return Promise.reject(new Error());
    inFlight.current = true;
    return gitPullRequestReview(project).finally(() => {
      inFlight.current = false;
    });
  }, [project]);
  const {
    value: review,
    loading,
    error: readError,
    refresh,
  } = useRepositoryRead(project == null ? null : `pull request review ${project}`, read);

  useEffect(() => {
    if (project == null) return;
    const every = readError === null ? REVIEW_POLL_MS : REVIEW_BACKOFF_MS;
    const timer = window.setInterval(() => {
      // A read that is still out is the answer this tick would have asked for, so asking again
      // would be two requests to a rate-limited service for one fact.
      if (!inFlight.current) refresh();
    }, every);
    return () => window.clearInterval(timer);
  }, [project, readError, refresh]);

  return useMemo(
    () => ({
      review,
      loading,
      error: actionError ?? handoffError ?? readError,
      merging: busy(MERGE),
      refresh,
      merge: (number, method) => {
        if (project == null) return Promise.resolve(false);
        return run(MERGE, () => gitMergePullRequest(project, number, method));
      },
      handOff: (subject, target) => {
        if (project == null) return Promise.resolve(null);
        setHandoffError(null);
        return gitHandOff(project, subject, target).catch((reason: unknown) => {
          setHandoffError(String(reason));
          return null;
        });
      },
    }),
    [actionError, busy, handoffError, loading, project, readError, refresh, review, run],
  );
}
