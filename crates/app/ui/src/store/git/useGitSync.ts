import { useCallback, useMemo, useRef } from "react";
import {
  gitAbortMerge,
  gitBranches,
  gitCreateBranch,
  gitDeleteBranch,
  gitFetch,
  gitPopStash,
  gitPull,
  gitPush,
  gitStash,
  gitStopExchange,
  gitSwitchBranch,
} from "@/api";
import { useGitActions } from "@/store/git/useGitActions";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";
import type { Branches } from "@/domain";

/** The keys each whole-repository action is tracked under. A branch name cannot collide with them. */
export const EXCHANGE = " exchange";
export const BRANCH = " branch";
export const STASH = " stash";
export const MERGE = " merge";

export interface GitSyncStore {
  /** The branches to offer and whether anything is stashed, or null until the read lands. */
  branches: Branches | null;
  /** Whether an exchange with the remote is under way, so it can be shown and stopped. */
  exchanging: boolean;
  /** Whether a branch or stash action is still running. */
  busy: (key: string) => boolean;
  /** What the last refused action said, for the surface to show until it is dismissed. */
  error: string | null;
  dismissError: () => void;
  fetch: () => void;
  pull: () => void;
  /** Hands the branch's commits over — publishing it when it tracks nothing, which the core decides. */
  push: () => void;
  /** Asks the exchange under way to stop; what comes back is not reported as a failure. */
  stopExchange: () => void;
  createBranch: (name: string) => Promise<boolean>;
  switchBranch: (name: string) => void;
  deleteBranch: (name: string) => void;
  stash: () => void;
  popStash: () => void;
  abortMerge: () => void;
}

/**
 * Everything a surface can do to a repository as a whole: exchange commits with its remote, move
 * between branches, and set the working tree's changes aside.
 *
 * Holds no rules of its own. Which exchange a branch without an upstream needs, whether a name is
 * usable, what version control would refuse, and whether a person may be asked for a credential are
 * every one of them the core's. This tracks what is in flight, what was last refused, and — for the
 * one action that can outlast anybody's patience — that it can be stopped.
 *
 * The branch read only runs while something is showing it (`listing`), the same way the file listing
 * does: a list nobody is looking at is a subprocess nobody needed.
 */
export function useGitSync(project: number | null, listing: boolean): GitSyncStore {
  const { busy, error, dismissError, run } = useGitActions();
  // Whether the exchange under way was stopped on purpose. Read when it comes back refused, so its
  // own account of itself is shown when it really failed and withheld when it was asked to stop.
  const stopped = useRef(false);

  const readBranches = useCallback(
    () => (project == null ? Promise.resolve(null) : gitBranches(project)),
    [project],
  );
  const { value: branches } = useRepositoryRead(
    project == null || !listing ? null : `branches ${project}`,
    readBranches,
  );

  return useMemo(() => {
    // A null project has nothing to change; the callbacks stay callable so a surface never has to
    // branch on it, and simply do nothing.
    const on = (key: string, action: (id: number) => Promise<void>, expected?: () => boolean) => {
      if (project == null) return Promise.resolve(false);
      return run(key, () => action(project), expected);
    };
    const exchange = (action: (id: number) => Promise<void>) => {
      stopped.current = false;
      return on(EXCHANGE, action, () => stopped.current);
    };
    return {
      branches,
      exchanging: busy(EXCHANGE),
      busy,
      error,
      dismissError,
      fetch: () => void exchange((id) => gitFetch(id)),
      pull: () => void exchange((id) => gitPull(id)),
      push: () => void exchange((id) => gitPush(id)),
      stopExchange: () => {
        if (project == null) return;
        stopped.current = true;
        void gitStopExchange(project);
      },
      createBranch: (name) => on(BRANCH, (id) => gitCreateBranch(id, name)),
      switchBranch: (name) => void on(BRANCH, (id) => gitSwitchBranch(id, name)),
      deleteBranch: (name) => void on(BRANCH, (id) => gitDeleteBranch(id, name)),
      stash: () => void on(STASH, (id) => gitStash(id)),
      popStash: () => void on(STASH, (id) => gitPopStash(id)),
      abortMerge: () => void on(MERGE, (id) => gitAbortMerge(id)),
    };
  }, [branches, busy, dismissError, error, project, run]);
}
