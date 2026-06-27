import { useCallback, useState } from "react";
import { mcpToolGroups, setMcpToolGroup } from "@/api";
import { DEFAULT_MCP_TOOL_GROUPS } from "@/lib/integrations";
import { persistThenReconcile } from "@/store/persist";
import { useLoadOnce } from "@/store/useLoadOnce";
import type { McpFeatureGroup, McpToolGroups } from "@/domain";

// The MCP feature-group enablement read model (G10): which tool groups the soloist-mcp server
// exposes. Loads once, then toggles one group at a time through the per-group facade setter
// (optimistic, reconciled from the echoed record). One group is its own write, so this can't
// reuse the whole-document useSettingsResource — only its load-once half.
export function useMcpToolGroups(): {
  groups: McpToolGroups;
  setGroup: (group: McpFeatureGroup, enabled: boolean) => void;
} {
  const [groups, setGroups] = useState<McpToolGroups>(DEFAULT_MCP_TOOL_GROUPS);

  useLoadOnce(mcpToolGroups, setGroups);

  const setGroup = useCallback((group: McpFeatureGroup, enabled: boolean) => {
    setGroups((prev) => ({ ...prev, [group]: enabled }));
    persistThenReconcile(setMcpToolGroup(group, enabled), mcpToolGroups, setGroups);
  }, []);

  return { groups, setGroup };
}
