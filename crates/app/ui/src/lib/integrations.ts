// The Integrations tab's display data and the MCP tool-group enablement contract. The per-group
// enablement is the real, enforced MCP control surface (G10): the soloist-mcp server composes
// only the enabled feature groups. The MCP transport (stdio, no network port — D4) is set up by
// pasting a generated per-client snippet; the local HTTP API surface is shown read-only so a
// script can be wired up.

import type { McpFeatureGroup, McpSetupInfo, McpToolGroups } from "@/domain";

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

// The name Soloist registers under in every client config.
export const MCP_SERVER_NAME = "soloist";

// Mirrors the Rust side's single source (`soloist_ipc::DATA_DIR_ENV`): the env var a snippet
// must carry when the data directory is overridden, or the helper resolves a different
// directory and misses the app's socket.
export const DATA_DIR_ENV = "SOLOIST_APP_DATA_DIR";

// The pre-load fallback for the snippet facts: the bare helper name (PATH lookup) and the
// default, un-overridden data directory. The app's resolved value supersedes this on load.
export const DEFAULT_MCP_SETUP_INFO: McpSetupInfo = {
  helper_path: "soloist-mcp",
  data_dir: "",
  data_dir_overridden: false,
};

// An MCP client the panel can generate a setup snippet for: its display label, where the
// snippet goes, and the renderer producing that client's exact config shape.
export interface McpClientInfo {
  id: McpClientId;
  label: string;
  configPath: string;
  snippet: (info: McpSetupInfo) => string;
}

export type McpClientId =
  | "claude-code"
  | "codex"
  | "amp"
  | "opencode"
  | "cursor"
  | "windsurf"
  | "cline";

// The env entry a snippet carries only when the data directory is overridden (matching how
// Soloist itself resolved it — see DATA_DIR_ENV).
function envEntry(info: McpSetupInfo): Record<string, string> | undefined {
  return info.data_dir_overridden ? { [DATA_DIR_ENV]: info.data_dir } : undefined;
}

// The common `mcpServers` JSON family (Claude Code, Cursor, Windsurf, Cline): command string,
// env object only when needed. Rendered via JSON.stringify so paths are always escaped validly.
function mcpServersJson(info: McpSetupInfo): string {
  const env = envEntry(info);
  return JSON.stringify(
    { mcpServers: { [MCP_SERVER_NAME]: { command: info.helper_path, ...(env && { env }) } } },
    null,
    2,
  );
}

// Amp keeps the same server shape but under its `amp.mcpServers` settings key (a settings.json
// fragment, not a whole file).
function ampSettingsJson(info: McpSetupInfo): string {
  const env = envEntry(info);
  return JSON.stringify(
    {
      "amp.mcpServers": { [MCP_SERVER_NAME]: { command: info.helper_path, ...(env && { env }) } },
    },
    null,
    2,
  );
}

// OpenCode diverges three ways from the common shape: top-level key `mcp`, `command` as an
// array, and the env key named `environment`; `"type": "local"` is required.
function opencodeJson(info: McpSetupInfo): string {
  const environment = envEntry(info);
  return JSON.stringify(
    {
      $schema: "https://opencode.ai/config.json",
      mcp: {
        [MCP_SERVER_NAME]: {
          type: "local",
          command: [info.helper_path],
          enabled: true,
          ...(environment && { environment }),
        },
      },
    },
    null,
    2,
  );
}

// Codex configures servers in TOML: an `[mcp_servers.<name>]` table, env in its own sub-table.
// Values are rendered with JSON string escaping, which is valid TOML basic-string escaping.
function codexToml(info: McpSetupInfo): string {
  const table = `[mcp_servers.${MCP_SERVER_NAME}]\ncommand = ${JSON.stringify(info.helper_path)}`;
  if (!info.data_dir_overridden) return table;
  return `${table}\n\n[mcp_servers.${MCP_SERVER_NAME}.env]\n${DATA_DIR_ENV} = ${JSON.stringify(info.data_dir)}`;
}

// Every client the panel offers, in display order. Each shape was verified against the client's
// official documentation; a client is one row here. Claude Desktop is deliberately absent: it
// ships for macOS/Windows only, and a stdio helper must run beside the app on this machine, so
// no working snippet exists for a Linux-only Soloist.
export const MCP_CLIENTS: McpClientInfo[] = [
  {
    id: "claude-code",
    label: "Claude Code",
    configPath: ".mcp.json (project root)",
    snippet: mcpServersJson,
  },
  {
    id: "codex",
    label: "Codex",
    configPath: "~/.codex/config.toml",
    snippet: codexToml,
  },
  {
    id: "amp",
    label: "Amp",
    configPath: "~/.config/amp/settings.json (or VS Code settings.json)",
    snippet: ampSettingsJson,
  },
  {
    id: "opencode",
    label: "OpenCode",
    configPath: "opencode.json (project root) or ~/.config/opencode/opencode.json",
    snippet: opencodeJson,
  },
  {
    id: "cursor",
    label: "Cursor",
    configPath: ".cursor/mcp.json (project) or ~/.cursor/mcp.json (global)",
    snippet: mcpServersJson,
  },
  {
    id: "windsurf",
    label: "Windsurf",
    configPath: "~/.codeium/windsurf/mcp_config.json",
    snippet: mcpServersJson,
  },
  {
    id: "cline",
    label: "Cline",
    configPath: "Cline panel → MCP Servers → Configure (cline_mcp_settings.json)",
    snippet: mcpServersJson,
  },
];

// The local HTTP API, shown read-only so a script can be pointed at it. Loopback only; mutations
// require the X-Soloist-Local-Auth header.
export const HTTP_API_BASE_URL = "http://127.0.0.1:24678";

// The documented HTTP surface (docs/http-api.md): six reads + thirteen actions. Listed so the user
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
  "POST /projects/:id/reload",
  "POST /projects/:id/spawn-agent",
  "POST /projects/:id/transfer-todo",
  "POST /projects/:id/transfer-scratchpad",
  "POST /focus",
];
