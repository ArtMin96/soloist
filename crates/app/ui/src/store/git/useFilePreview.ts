import { useCallback } from "react";
import { gitFile } from "@/api";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";
import type { FileContent } from "@/domain";

export interface FilePreviewStore {
  /** The file, or null for a path that is gone and until the first read lands. */
  content: FileContent | null;
  loading: boolean;
  error: string | null;
}

/**
 * The working tree's copy of one path, for the surface that shows a file rather than a change to
 * one. Read through the façade like every other repository read — nothing here touches a
 * filesystem — and re-read whenever version control says something changed.
 */
export function useFilePreview(project: number | null, path: string | null): FilePreviewStore {
  const read = useCallback(
    () => (project == null || path === null ? Promise.resolve(null) : gitFile(project, path)),
    [path, project],
  );
  const { value, loading, error } = useRepositoryRead(
    project == null || path === null ? null : `${project} ${path}`,
    read,
  );
  return { content: value, loading, error };
}
