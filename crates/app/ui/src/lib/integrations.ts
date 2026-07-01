// The Integrations tab's display data and the MCP tool-group enablement contract. The per-group
// enablement is the real, enforced MCP control surface (G10): the soloist-mcp server composes
// only the enabled feature groups. The MCP transport (stdio, no network port — D4) and the
// local HTTP API surface are shown read-only so a client or script can be wired up.

import type { McpFeatureGroup, McpToolGroups } from "@/domain";

// The pre-load fallback for the enablement: Scratchpads / Todos / Timers default on, Key-Value
// off (the G10 contract). The facade's stored value supersedes this on load.
export const DEFAULT_MCP_TOOL_GROUPS: McpToolGroups = {
  scratchpads: true,
  todos: true,
  timers: true,
  key_value: false,
};

export interface McpGroupInfo {
  group: McpFeatureGroup;
  label: string;
  description: string;
}

// The toggleable MCP feature-tool groups, in served order. Core tool groups are always exposed
// and are not listed here. One source for the panel's rows.
export const MCP_TOOL_GROUPS: McpGroupInfo[] = [
  {
    group: "scratchpads",
    label: "Scratchpads",
    description: "Shared structured documents agents read and write.",
  },
  {
    group: "todos",
    label: "Todos",
    description: "Shared work items with blockers and process-owned locks.",
  },
  {
    group: "timers",
    label: "Timers",
    description: "Fire-when-idle and scheduled agent wake-ups.",
  },
  {
    group: "key_value",
    label: "Key-Value",
    description: "Small shared key-value state. Off by default.",
  },
];

// The MCP server speaks over stdio (no network port); a client launches this command and talks
// to it on stdin/stdout. The standard MCP stdio-server config shape registers it.
export const MCP_CLIENT_CONFIG = `{
  "mcpServers": {
    "soloist": { "command": "soloist-mcp" }
  }
}`;

// The local HTTP API, shown read-only so a script can be pointed at it. Loopback only; mutations
// require the X-Soloist-Local-Auth header.
export const HTTP_API_BASE_URL = "http://127.0.0.1:24678";

// The documented HTTP surface (docs/http-api.md): six reads + ten actions. Listed so the user
// can see exactly what a local script can reach; the count is derived, never a magic number.
export const HTTP_API_ENDPOINTS: string[] = [
  "GET  /health",
  "GET  /status",
  "GET  /processes",
  "GET  /processes/:id/ports",
  "GET  /processes/:id/output",
  "GET  /projects",
  "POST /processes/:id/start",
  "POST /processes/:id/stop",
  "POST /processes/:id/restart",
  "POST /projects/:id/start-auto",
  "POST /projects/:id/start-all",
  "POST /projects/:id/stop-all",
  "POST /projects/:id/restart-running",
  "POST /projects/:id/restart-all",
  "POST /projects/:id/spawn-agent",
  "POST /focus",
];
