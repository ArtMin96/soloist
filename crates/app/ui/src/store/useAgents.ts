import { useCallback, useState } from "react";
import { agentDetect, agentLaunch, agentList } from "@/api";
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
// launch action. `reload` lists immediately so the picker opens instantly, then merges the
// `--version` detection so installed badges fill in without blocking the list. `launch`
// routes to the single core behaviour; a failure surfaces via `onError` and resolves null so
// the caller simply no-ops rather than throwing.
export function useAgents(onError: (message: string) => void): AgentStore {
  const [tools, setTools] = useState<DetectedTool[]>([]);

  const reload = useCallback(() => {
    agentList()
      .then((list) => {
        // Render the list at once as not-yet-detected, then refine with the probe result.
        setTools(list.map((tool) => ({ tool, installed: false })));
        return agentDetect();
      })
      .then(setTools)
      .catch(() => {
        // A failed list/detect just leaves the picker empty; it is never fatal.
      });
  }, []);

  const launch = useCallback(
    (project: number, tool: string, extraArgs: string[]): Promise<number | null> =>
      agentLaunch(project, tool, extraArgs).catch((error: unknown) => {
        onError(String(error));
        return null;
      }),
    [onError],
  );

  return { tools, reload, launch };
}
