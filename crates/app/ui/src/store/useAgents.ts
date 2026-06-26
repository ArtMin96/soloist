import { useCallback } from "react";
import { agentDetect, agentLaunch, agentList } from "@/api";
import { CacheKey } from "@/store/cache/persistentCache";
import { usePersistentSnapshot } from "@/store/cache/usePersistentSnapshot";
import type { DetectedTool } from "@/domain";

export interface AgentStore {
  /** The configured agent tools with detection status; empty until `reload` runs. */
  tools: DetectedTool[];
  /** Lists the tools instantly (no probing), then fills in `--version` detection. */
  reload: () => void;
  /**
   * Launches `tool` in `project` with `extraArgs`, resolving to the new process id — or
   * `null` if the launch failed (the error is surfaced via `onError`).
   */
  launch: (project: number, tool: string, extraArgs: string[]) => Promise<number | null>;
}

// The agents read-model (C4) on the frontend: the tools the launch picker shows and the one
// launch action. The detected tools are a persisted stale-while-revalidate snapshot, so a
// warm picker opens instantly against the last-known detection; revalidation runs only on
// `reload` (the picker opening), never at launch, so opening the app probes no CLIs. On a
// cold open the fetcher lists the tools at once (not-yet-detected) and then refines them with
// the `--version` probe, so the list never blocks on detection. `launch` routes to the single
// core behaviour; a failure surfaces via `onError` and resolves null so the caller no-ops.
export function useAgents(onError: (message: string) => void): AgentStore {
  const { value, revalidate } = usePersistentSnapshot<DetectedTool[]>(
    CacheKey.agents,
    async (emit) => {
      const list = await agentList();
      emit(list.map((tool) => ({ tool, installed: false })));
      return agentDetect();
    },
    { revalidateOnMount: false },
  );

  const launch = useCallback(
    (project: number, tool: string, extraArgs: string[]): Promise<number | null> =>
      agentLaunch(project, tool, extraArgs).catch((error: unknown) => {
        onError(String(error));
        return null;
      }),
    [onError],
  );

  return { tools: value ?? [], reload: revalidate, launch };
}
