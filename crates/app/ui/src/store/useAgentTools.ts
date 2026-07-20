import { useAgentDetection } from "@/store/useAgentDetection";
import type { DetectedTool } from "@/domain";

// The detected agent tools for the Agents settings registry — the same shared, cached detection
// the launch picker uses (one source, no re-rolled probe). The Agents tab revalidates when it
// opens (the snapshot's default), so the panel shows last-known badges instantly and reconciles
// on open. `detect` is the explicit button: it re-probes the CLIs rather than reading through the
// core's cached sweep, so a wrong result is always correctable on demand.
export function useAgentTools(): { tools: DetectedTool[]; detect: () => void } {
  const { tools, refresh } = useAgentDetection();
  return { tools, detect: refresh };
}
