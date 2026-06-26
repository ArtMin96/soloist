import { useCallback, useEffect, useState } from "react";
import { mcpToolGroups, setMcpToolGroup } from "@/api";
import { DEFAULT_MCP_TOOL_GROUPS } from "@/lib/integrations";
import type { McpFeatureGroup, McpToolGroups } from "@/domain";

// The MCP feature-group enablement read model (G10): which tool groups the soloist-mcp server
// exposes. Loads once, then toggles one group at a time through the per-group facade setter
// (optimistic, reconciled from the echoed record). One group is its own write, so this can't
// reuse the whole-document useSettingsResource.
export function useMcpToolGroups(): {
  groups: McpToolGroups;
  setGroup: (group: McpFeatureGroup, enabled: boolean) => void;
} {
  const [groups, setGroups] = useState<McpToolGroups>(DEFAULT_MCP_TOOL_GROUPS);

  useEffect(() => {
    let cancelled = false;
    mcpToolGroups()
      .then((loaded) => {
        if (!cancelled) setGroups(loaded);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  const setGroup = useCallback((group: McpFeatureGroup, enabled: boolean) => {
    setGroups((prev) => ({ ...prev, [group]: enabled }));
    void setMcpToolGroup(group, enabled)
      .then(setGroups)
      .catch(() => {});
  }, []);

  return { groups, setGroup };
}
