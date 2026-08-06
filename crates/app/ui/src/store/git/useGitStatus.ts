import { useCallback } from "react";
import { gitStatus } from "@/api";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";
import type { GitStatus } from "@/domain";

export interface GitStatusStore {
  /** The working tree, or null for a project that is not a repository. */
  status: GitStatus | null;
  /** True until the first read for this project resolves — the rail waits rather than claiming
   *  a repository has no changes when it has simply not been read yet. */
  loading: boolean;
  error: string | null;
  refresh: () => void;
}

/**
 * A project's working-tree status, seeded from the snapshot and re-read whenever version control
 * says something changed. A null project clears everything, and a status captured for another
 * project is never shown for this one.
 */
export function useGitStatus(project: number | null): GitStatusStore {
  const read = useCallback(
    () => (project == null ? Promise.resolve(null) : gitStatus(project)),
    [project],
  );
  const { value, loading, error, refresh } = useRepositoryRead(
    project == null ? null : String(project),
    read,
  );
  return { status: value, loading, error, refresh };
}
