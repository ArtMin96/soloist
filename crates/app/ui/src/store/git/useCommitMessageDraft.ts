import { useCallback, useMemo, useState } from "react";
import { assistSettings, gitDraftCommitMessage } from "@/api";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";

export interface CommitMessageDraftStore {
  /** Whether a tool is configured to draft with, so the affordance exists at all. */
  available: boolean;
  /** Whether a draft is still being written — an agent takes a moment to answer. */
  drafting: boolean;
  /** What the last refused draft said, for the surface to show until it is dismissed. */
  error: string | null;
  dismissError: () => void;
  /** Resolves the drafted message, or null when none was drafted. */
  draft: () => Promise<string | null>;
}

/**
 * Asking for a commit message to be drafted, and whether that can be asked for at all.
 *
 * Holds no rules of its own. Which tool runs, whether the project permits it, what the agent is
 * shown and what is left out are every one of them the core's; this tracks whether a run is in
 * flight and what the last refusal said. The one thing it reads is whether a tool is selected —
 * asked so the affordance can be absent rather than offered and then refused.
 *
 * The read rides the same refresh every other repository read does, so turning the feature on is
 * noticed without the rail being rebuilt.
 */
export function useCommitMessageDraft(project: number | null): CommitMessageDraftStore {
  const [drafting, setDrafting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // The selection is global rather than per project, so the request never mentions one.
  const readAssist = useCallback(() => assistSettings(), []);
  const { value: assist } = useRepositoryRead("assist", readAssist);

  return useMemo(
    () => ({
      available: assist?.tool != null,
      drafting,
      error,
      dismissError: () => setError(null),
      draft: () => {
        // A null project has nothing to describe; the callback stays callable so a surface never
        // has to branch on it.
        if (project == null) return Promise.resolve(null);
        setDrafting(true);
        setError(null);
        return gitDraftCommitMessage(project)
          .catch((reason: unknown) => {
            setError(String(reason));
            return null;
          })
          .finally(() => setDrafting(false));
      },
    }),
    [assist, drafting, error, project],
  );
}
