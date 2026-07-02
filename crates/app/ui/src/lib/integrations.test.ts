import { describe, expect, it } from "vitest";
import { DATA_DIR_ENV, MCP_CLIENTS, MCP_SERVER_NAME } from "@/lib/integrations";
import type { McpSetupInfo } from "@/domain";

const DEFAULT_INFO: McpSetupInfo = {
  helper_path: "/usr/bin/soloist-mcp",
  data_dir: "/home/u/.local/share/soloist",
  data_dir_overridden: false,
};

const OVERRIDDEN_INFO: McpSetupInfo = {
  helper_path: "/usr/bin/soloist-mcp",
  data_dir: "/custom/data dir",
  data_dir_overridden: true,
};

function client(id: string) {
  const found = MCP_CLIENTS.find((c) => c.id === id);
  if (!found) throw new Error(`no client ${id}`);
  return found;
}

describe("MCP client snippets", () => {
  it("every client's snippet carries the resolved helper command", () => {
    for (const c of MCP_CLIENTS) {
      expect(c.snippet(DEFAULT_INFO), c.id).toContain(DEFAULT_INFO.helper_path);
    }
  });

  it("every client's snippet carries the data-dir env var exactly when overridden", () => {
    for (const c of MCP_CLIENTS) {
      expect(c.snippet(DEFAULT_INFO), c.id).not.toContain(DATA_DIR_ENV);
      const overridden = c.snippet(OVERRIDDEN_INFO);
      expect(overridden, c.id).toContain(DATA_DIR_ENV);
      expect(overridden, c.id).toContain("/custom/data dir");
    }
  });

  it("the mcpServers-family snippets are valid JSON with the documented shape", () => {
    for (const id of ["claude-code", "cursor", "windsurf", "cline"]) {
      const parsed = JSON.parse(client(id).snippet(OVERRIDDEN_INFO)) as {
        mcpServers: Record<string, { command: string; env: Record<string, string> }>;
      };
      const server = parsed.mcpServers[MCP_SERVER_NAME];
      expect(server.command, id).toBe(OVERRIDDEN_INFO.helper_path);
      expect(server.env[DATA_DIR_ENV], id).toBe(OVERRIDDEN_INFO.data_dir);
    }
  });

  it("the amp snippet nests the same server shape under the amp.mcpServers settings key", () => {
    const parsed = JSON.parse(client("amp").snippet(DEFAULT_INFO)) as Record<
      string,
      Record<string, { command: string; env?: unknown }>
    >;
    const server = parsed["amp.mcpServers"][MCP_SERVER_NAME];
    expect(server.command).toBe(DEFAULT_INFO.helper_path);
    expect(server.env).toBeUndefined();
  });

  it("the opencode snippet uses its own keys: mcp, an argv command array, and environment", () => {
    const parsed = JSON.parse(client("opencode").snippet(OVERRIDDEN_INFO)) as {
      mcp: Record<
        string,
        { type: string; command: string[]; enabled: boolean; environment: Record<string, string> }
      >;
    };
    const server = parsed.mcp[MCP_SERVER_NAME];
    expect(server.type).toBe("local");
    expect(server.command).toEqual([OVERRIDDEN_INFO.helper_path]);
    expect(server.enabled).toBe(true);
    expect(server.environment[DATA_DIR_ENV]).toBe(OVERRIDDEN_INFO.data_dir);
  });

  it("the codex snippet is a TOML mcp_servers table, env in its own sub-table only when overridden", () => {
    expect(client("codex").snippet(DEFAULT_INFO)).toBe(
      `[mcp_servers.${MCP_SERVER_NAME}]\ncommand = "/usr/bin/soloist-mcp"`,
    );
    expect(client("codex").snippet(OVERRIDDEN_INFO)).toBe(
      `[mcp_servers.${MCP_SERVER_NAME}]\ncommand = "/usr/bin/soloist-mcp"\n\n[mcp_servers.${MCP_SERVER_NAME}.env]\n${DATA_DIR_ENV} = "/custom/data dir"`,
    );
  });

  it("a helper path needing escaping still renders valid JSON", () => {
    const awkward: McpSetupInfo = {
      helper_path: '/opt/"quoted" apps/soloist-mcp',
      data_dir: "",
      data_dir_overridden: false,
    };
    const parsed = JSON.parse(client("claude-code").snippet(awkward)) as {
      mcpServers: Record<string, { command: string }>;
    };
    expect(parsed.mcpServers[MCP_SERVER_NAME].command).toBe(awkward.helper_path);
  });
});
