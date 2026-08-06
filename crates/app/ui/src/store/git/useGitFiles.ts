import { useCallback } from "react";
import { gitFiles } from "@/api";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";
import type { ProjectFile } from "@/domain";

export interface GitFilesStore {
  /** Every path in the repository, or null for a project that is not a repository. */
  files: ProjectFile[] | null;
  /** True until the first read for this project resolves. */
  loading: boolean;
  error: string | null;
}

/**
 * A project's file listing, read only while `active` — the whole tree costs a walk of the
 * working tree, so it is read when something is showing it and not before.
 */
export function useGitFiles(project: number | null, active: boolean): GitFilesStore {
  const read = useCallback(
    () => (project == null ? Promise.resolve(null) : gitFiles(project)),
    [project],
  );
  const { value, loading, error } = useRepositoryRead(
    project == null || !active ? null : String(project),
    read,
  );
  return { files: value, loading, error };
}
