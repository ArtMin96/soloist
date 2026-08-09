import { useCallback, useMemo, useState } from "react";
import { gitCreatePullRequest, gitDraftPullRequestBody, gitPullRequestSurface } from "@/api";
import { useGitActions } from "@/store/git/useGitActions";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";
import type { NewPullRequest, PullRequestSurface } from "@/domain";

/** The key the one project-wide action is tracked under. */
const PROPOSE = "";

export interface PullRequestStore {
  /** What can be offered, or null until the first read lands. */
  surface: PullRequestSurface | null;
  /** True until that first read resolves. */
  loading: boolean;
  /** What the last refused action or failed read said, for the surface to show. */
  error: string | null;
  dismissError: () => void;
  /** Whether a proposal is still being made — it pushes and then reaches a service. */
  proposing: boolean;
  /** Whether a description is still being drafted — an agent takes a moment to answer. */
  drafting: boolean;
  /** Resolves the address of what was made, or null when it was refused. */
  propose: (request: NewPullRequest) => Promise<string | null>;
  /** Resolves the drafted description, or null when none was drafted. */
  draft: (base: string, skeleton: string) => Promise<string | null>;
}

/**
 * What the pull-request surface can offer, and the two things it can ask for.
 *
 * Holds no rules of its own. Whether the forge can be reached, which description skeleton wins,
 * whether the branch has to be pushed first, and whether the project may be acted on at all are
 * every one of them the core's; this tracks what is in flight and what the last refusal said.
 *
 * The read rides the same refresh every other repository read does, so signing the tool in — or
 * a proposal landing — is noticed without the pane being rebuilt.
 */
export function usePullRequest(project: number | null): PullRequestStore {
  const { busy, error: actionError, dismissError: dismissAction, run } = useGitActions();
  const [drafting, setDrafting] = useState(false);
  const [draftError, setDraftError] = useState<string | null>(null);

  const read = useCallback(
    () => (project == null ? Promise.reject(new Error()) : gitPullRequestSurface(project)),
    [project],
  );
  const {
    value: surface,
    loading,
    error: readError,
  } = useRepositoryRead(project == null ? null : `pull request ${project}`, read);

  return useMemo(
    () => ({
      surface,
      loading,
      error: actionError ?? draftError ?? readError,
      dismissError: () => {
        dismissAction();
        setDraftError(null);
      },
      proposing: busy(PROPOSE),
      drafting,
      propose: async (request) => {
        // A null project has nothing to propose; the callback stays callable so a surface never
        // has to branch on it.
        if (project == null) return null;
        let made: string | null = null;
        await run(PROPOSE, async () => {
          made = await gitCreatePullRequest(project, request);
        });
        return made;
      },
      draft: (base, skeleton) => {
        if (project == null) return Promise.resolve(null);
        setDrafting(true);
        setDraftError(null);
        return gitDraftPullRequestBody(project, base, skeleton)
          .catch((reason: unknown) => {
            setDraftError(String(reason));
            return null;
          })
          .finally(() => setDrafting(false));
      },
    }),
    [
      actionError,
      busy,
      dismissAction,
      draftError,
      drafting,
      loading,
      project,
      readError,
      run,
      surface,
    ],
  );
}
