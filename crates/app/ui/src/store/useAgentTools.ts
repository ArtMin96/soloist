import { useAgentDetection } from "@/store/useAgentDetection";
import type { DetectedTool } from "@/domain";

// The detected agent tools for the Agents settings registry — the same shared, cached detection
// the launch picker uses (one source, no re-rolled probe). The Agents tab revalidates when it
// opens (the snapshot's default), so the panel shows last-known badges instantly and re-probes on
// open; `detect` re-runs that probe on demand.
export function useAgentTools(): { tools: DetectedTool[]; detect: () => void } {
  const { tools, revalidate } = useAgentDetection();
  return { tools, detect: revalidate };
}
