import { useCallback, useRef } from "react";
import { agentDetect, agentList, agentRedetect } from "@/api";
import { CacheKey } from "@/store/cache/persistentCache";
import { usePersistentSnapshot, type SnapshotOptions } from "@/store/cache/usePersistentSnapshot";
import type { DetectedTool } from "@/domain";

// The detected agent tools (C4) as a persisted stale-while-revalidate snapshot: a warm open
// paints the last-known badges instantly, then a revalidation re-runs the off-runtime
// `--version` probe — the slowest read in the stack — and reconciles. The backend always wins,
// so there is no stale risk; a PATH change is picked up by the next revalidate. The cold-open
// fetcher lists the tools first (not yet checked) and then refines them, so the list never
// blocks on detection. One detection path shared by the launch picker and the Agents settings
// registry, so the probe and its cache key live in a single place.
//
// `revalidate` reads through the core's cached sweep; `refresh` is the explicit user action and
// re-probes regardless of that cache, so a "detect" that finds nothing can always be retried.
export function useAgentDetection(options?: SnapshotOptions): {
  tools: DetectedTool[];
  revalidate: () => void;
  refresh: () => void;
} {
  // Set for exactly one fetch, so only the run a `refresh` triggered bypasses the core's cache.
  const reprobeNextFetch = useRef(false);

  const { value, revalidate } = usePersistentSnapshot<DetectedTool[]>(
    CacheKey.agents,
    async (emit) => {
      if (reprobeNextFetch.current) {
        reprobeNextFetch.current = false;
        // The tools are already on screen; only their detection is being re-resolved.
        return agentRedetect();
      }
      const list = await agentList();
      emit(list.map((tool) => ({ tool, detection: "Unknown" as const })));
      return agentDetect();
    },
    options,
  );

  const refresh = useCallback(() => {
    reprobeNextFetch.current = true;
    revalidate();
  }, [revalidate]);

  return { tools: value ?? [], revalidate, refresh };
}
