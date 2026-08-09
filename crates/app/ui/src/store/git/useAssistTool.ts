import { useCallback } from "react";
import { assistSettings } from "@/api";
import { useRepositoryRead } from "@/store/git/useRepositoryRead";

/**
 * Whether a tool is configured to draft text with at all.
 *
 * Asked so an affordance can be absent rather than offered and then refused, and asked in one
 * place because both things that can be drafted — a commit message and a pull request's
 * description — run the same tool under the same selection.
 *
 * The selection is global rather than per project, so the request never mentions one. The read
 * rides the same refresh every other repository read does, so turning the feature on is noticed
 * without the surface being rebuilt.
 */
export function useAssistTool(): boolean {
  const readAssist = useCallback(() => assistSettings(), []);
  const { value } = useRepositoryRead("assist", readAssist);
  return value?.tool != null;
}
