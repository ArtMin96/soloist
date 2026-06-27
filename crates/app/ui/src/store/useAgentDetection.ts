import { agentDetect, agentList } from "@/api";
import { CacheKey } from "@/store/cache/persistentCache";
import { usePersistentSnapshot, type SnapshotOptions } from "@/store/cache/usePersistentSnapshot";
import type { DetectedTool } from "@/domain";

// The detected agent tools (C4) as a persisted stale-while-revalidate snapshot: a warm open
// paints the last-known installed badges instantly, then a revalidation re-runs the off-runtime
// `--version` probe — the slowest read in the stack — and reconciles. The backend always wins,
// so there is no stale risk; a PATH change is picked up by the next revalidate. The cold-open
// fetcher lists the tools first (not-yet-detected) and then refines them, so the list never
// blocks on detection. One detection path shared by the launch picker and the Agents settings
// registry, so the probe and its cache key live in a single place.
export function useAgentDetection(options?: SnapshotOptions): {
  tools: DetectedTool[];
  revalidate: () => void;
} {
  const { value, revalidate } = usePersistentSnapshot<DetectedTool[]>(
    CacheKey.agents,
    async (emit) => {
      const list = await agentList();
      emit(list.map((tool) => ({ tool, installed: false })));
      return agentDetect();
    },
    options,
  );
  return { tools: value ?? [], revalidate };
}
