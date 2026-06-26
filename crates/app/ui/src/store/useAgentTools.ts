import { useCallback, useEffect, useState } from "react";
import { agentDetect, agentList } from "@/api";
import type { DetectedTool } from "@/domain";

// The configured agent tools with installed-detection, for the Agents settings registry. Lists
// instantly (not-yet-detected), then merges the `--version` probe — the same two-step the launch
// picker uses, without the launch action. Re-runnable from the panel via `detect`.
export function useAgentTools(): { tools: DetectedTool[]; detect: () => void } {
  const [tools, setTools] = useState<DetectedTool[]>([]);

  const detect = useCallback(() => {
    agentList()
      .then((list) => {
        setTools(list.map((tool) => ({ tool, installed: false })));
        return agentDetect();
      })
      .then(setTools)
      .catch(() => {});
  }, []);

  useEffect(() => detect(), [detect]);

  return { tools, detect };
}
