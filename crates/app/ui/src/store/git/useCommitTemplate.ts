import { useCallback } from "react";
import { gitCommitTemplate } from "@/api";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";

/**
 * The message a new commit starts from, as the repository's own configuration supplies it, or
 * null where it supplies none — which is what most repositories do.
 *
 * Asked only where the project has been trusted, because that is where the core answers it: the
 * configuration behind it is one the repository itself can carry. Asked separately from the
 * changes a surface can make, so the diff view — which needs those and not this — spends nothing
 * on it.
 *
 * The read rides the same refresh every other repository read does, so a template configured
 * while the app is open is picked up without it being restarted.
 */
export function useCommitTemplate(project: number | null, trusted: boolean): string | null {
  const read = useCallback(
    () => (project == null ? Promise.resolve(null) : gitCommitTemplate(project)),
    [project],
  );
  const { value } = useRepositoryRead(
    project == null || !trusted ? null : `template ${project}`,
    read,
  );
  return value;
}
