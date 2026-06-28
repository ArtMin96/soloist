import { useCallback } from "react";
import { agentLaunch } from "@/api";
import { useAgentDetection } from "@/store/useAgentDetection";
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

// The agents read-model (C4) for the launch picker: the detected tools plus the one launch
// action. Detection runs through the shared `useAgentDetection` cache; the picker never probes
// at startup (`revalidateOnMount: false`) — it seeds from the last-known snapshot and revalidates
// only when opened (`reload`), so launching the app probes no CLIs. `launch` routes to the single
// core behaviour; a failure surfaces via `onError` and resolves null so the caller no-ops.
export function useAgents(onError: (message: string) => void): AgentStore {
  const { tools, revalidate } = useAgentDetection({ revalidateOnMount: false });

  const launch = useCallback(
    (project: number, tool: string, extraArgs: string[]): Promise<number | null> =>
      agentLaunch(project, tool, extraArgs).catch((error: unknown) => {
        onError(String(error));
        return null;
      }),
    [onError],
  );

  return { tools, reload: revalidate, launch };
}
