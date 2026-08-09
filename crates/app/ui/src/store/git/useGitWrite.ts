import { useCallback, useMemo } from "react";
import {
  gitCommit,
  gitDiscard,
  gitDiscardHunk,
  gitOpenFile,
  gitStage,
  gitStageHunk,
  gitTrusted,
  gitTrustProject,
  gitUnstage,
  gitUnstageHunk,
} from "@/api";
import { hunkKey } from "@/lib/git";
import { useGitActions } from "@/store/git/useGitActions";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";
import type { HunkRange } from "@/domain";

export interface GitWriteStore {
  /** Whether the user has trusted this project to be changed, or null until the read lands. */
  trusted: boolean | null;
  /** Records that trust, which is what the affordance behind `trusted === false` does. */
  trust: () => void;
  /** What the last refused action said, for the surface to show until it is dismissed. */
  error: string | null;
  dismissError: () => void;
  /** Whether an action on this path or hunk is still running. */
  busy: (key: string) => boolean;
  /** Whether a commit is still running — a hook of the user's may take a moment. */
  committing: boolean;
  stage: (path: string) => void;
  unstage: (path: string) => void;
  discard: (path: string) => void;
  stageHunk: (path: string, hunk: HunkRange) => void;
  unstageHunk: (path: string, hunk: HunkRange) => void;
  discardHunk: (path: string, hunk: HunkRange) => void;
  /** Resolves true when the commit was recorded, false when it was refused. */
  commit: (message: string, amend: boolean) => Promise<boolean>;
  /** Hands one path to whatever this machine has registered to open it. */
  open: (path: string) => void;
}

/**
 * Every change a surface can make to one path of a repository, and whether it is allowed to make
 * one.
 *
 * Holds no rules of its own: the core decides what trust permits, what a discard may reach, and
 * whether a hunk is still there. What is in flight and what was last refused are tracked by
 * `useGitActions`, shared with the surfaces that change the repository as a whole.
 *
 * Nothing here re-reads the status afterwards: a change that lands announces itself from the
 * core, and every repository surface is already listening for that.
 */
export function useGitWrite(project: number | null): GitWriteStore {
  const { busy, error, dismissError, run } = useGitActions();

  const readTrust = useCallback(
    () => (project == null ? Promise.resolve(null) : gitTrusted(project)),
    [project],
  );
  const { value: trusted } = useRepositoryRead(
    project == null ? null : `trusted ${project}`,
    readTrust,
  );

  return useMemo(() => {
    // A null project has nothing to change; the callbacks stay callable so a surface never has
    // to branch on it, and simply do nothing.
    const on = (key: string, action: (id: number) => Promise<void>) => {
      if (project == null) return Promise.resolve(false);
      return run(key, () => action(project));
    };
    return {
      trusted,
      trust: () => void on(TRUST, (id) => gitTrustProject(id)),
      error,
      dismissError,
      busy,
      committing: busy(COMMIT),
      stage: (path) => void on(path, (id) => gitStage(id, path)),
      unstage: (path) => void on(path, (id) => gitUnstage(id, path)),
      discard: (path) => void on(path, (id) => gitDiscard(id, path)),
      stageHunk: (path, hunk) => void on(hunkKey(path, hunk), (id) => gitStageHunk(id, path, hunk)),
      unstageHunk: (path, hunk) =>
        void on(hunkKey(path, hunk), (id) => gitUnstageHunk(id, path, hunk)),
      discardHunk: (path, hunk) =>
        void on(hunkKey(path, hunk), (id) => gitDiscardHunk(id, path, hunk)),
      commit: (message, amend) => on(COMMIT, (id) => gitCommit(id, message, amend)),
      open: (path) => void on(path, (id) => gitOpenFile(id, path)),
    };
  }, [busy, dismissError, error, project, run, trusted]);
}

/** The keys the two project-wide actions are tracked under; no path can collide with them. */
const TRUST = "";
const COMMIT = " commit";
