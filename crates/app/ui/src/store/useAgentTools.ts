import { useCallback, useState } from "react";
import { detectionFailure } from "@/lib/agents";
import { useAgentDetection } from "@/store/useAgentDetection";
import type { DetectedTool } from "@/domain";

// The detected agent tools for the Agents settings registry — the same shared, cached detection
// the launch picker uses (one source, no re-rolled probe). The Agents tab revalidates when it
// opens (the snapshot's default), so the panel shows last-known badges instantly and reconciles
// on open. `detect` is the explicit button: it re-probes the CLIs rather than reading through the
// core's cached sweep, so a wrong result is always correctable on demand.
//
// A sweep that never answered is reported rather than swallowed. Without it every badge sits at
// "not checked" and the drafting picker offers nothing, which reads as a machine with no agent CLIs
// on it — the one wrong conclusion the Installed/Missing/Unknown distinction exists to prevent.
export function useAgentTools(): {
  tools: DetectedTool[];
  detect: () => void;
  /** Why the last sweep produced no answer, or null when it answered. */
  failure: string | null;
} {
  const [failure, setFailure] = useState<string | null>(null);
  const { tools, refresh } = useAgentDetection({
    onError: (reason) => setFailure(detectionFailure(reason)),
  });

  // Clearing before the re-probe rather than on its result: the sweep either answers (and the
  // failure is over) or fails again (and says so), so there is no third outcome to leave it up for.
  const detect = useCallback(() => {
    setFailure(null);
    refresh();
  }, [refresh]);

  return { tools, detect, failure };
}
