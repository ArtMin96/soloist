// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { IntegrationsPanel } from "@/components/settings/IntegrationsPanel";
import { DEFAULT_MCP_TOOL_GROUPS, HTTP_API_ENDPOINTS, MCP_TOOL_GROUPS } from "@/lib/integrations";
import type { McpFeatureGroup, McpSetupInfo, McpToolGroups } from "@/domain";

const setupInfo: McpSetupInfo = {
  helper_path: "/usr/bin/soloist-mcp",
  data_dir: "/home/u/.local/share/soloist",
  data_dir_overridden: false,
};

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Settings — Integrations", () => {
  it("loads the MCP tool-group enablement and toggles a group through the per-group setter", async () => {
    let lastSet: { group: McpFeatureGroup; enabled: boolean } | null = null;
    const groups: McpToolGroups = { ...DEFAULT_MCP_TOOL_GROUPS };
    mockIPC((cmd, args) => {
      if (cmd === "mcp_tool_groups") return groups;
      if (cmd === "mcp_setup_info") return setupInfo;
      if (cmd === "set_mcp_tool_group") {
        const next = args as { group: McpFeatureGroup; enabled: boolean };
        lastSet = next;
        return { ...groups, [next.group]: next.enabled };
      }
      return undefined;
    });

    render(<IntegrationsPanel />);

    // Key-Value loads off (the G10 default); enabling it routes through set_mcp_tool_group.
    const keyValue = await screen.findByRole("switch", { name: "Key-Value" });
    await waitFor(() => expect(keyValue.getAttribute("aria-checked")).toBe("false"));
    fireEvent.click(keyValue);
    await waitFor(() => expect(lastSet).toEqual({ group: "key_value", enabled: true }));
  });

  it("offers a switch for every feature group the core defines, so none is unreachable", async () => {
    mockIPC((cmd) => {
      if (cmd === "mcp_tool_groups") return DEFAULT_MCP_TOOL_GROUPS;
      if (cmd === "mcp_setup_info") return setupInfo;
      return undefined;
    });

    render(<IntegrationsPanel />);

    // The enablement record must name every group (the compiler sees to that), so its keys are an
    // independent list of what exists — while the rows are hand-written and can silently omit one.
    // A group with no row is a setting nobody can reach, which is what this asserts against.
    const defined = Object.keys(DEFAULT_MCP_TOOL_GROUPS) as McpFeatureGroup[];
    const labelled = new Map(MCP_TOOL_GROUPS.map((info) => [info.group, info.label]));
    for (const group of defined) {
      const label = labelled.get(group);
      expect(label, `${group} has no row in the panel`).toBeTruthy();
      expect(await screen.findByRole("switch", { name: label })).toBeTruthy();
    }
  });

  it("loads Git off by default and turns it on through the per-group setter", async () => {
    let lastSet: { group: McpFeatureGroup; enabled: boolean } | null = null;
    mockIPC((cmd, args) => {
      if (cmd === "mcp_tool_groups") return DEFAULT_MCP_TOOL_GROUPS;
      if (cmd === "mcp_setup_info") return setupInfo;
      if (cmd === "set_mcp_tool_group") {
        const next = args as { group: McpFeatureGroup; enabled: boolean };
        lastSet = next;
        return { ...DEFAULT_MCP_TOOL_GROUPS, [next.group]: next.enabled };
      }
      return undefined;
    });

    render(<IntegrationsPanel />);

    const git = await screen.findByRole("switch", { name: "Git" });
    await waitFor(() => expect(git.getAttribute("aria-checked")).toBe("false"));
    fireEvent.click(git);
    await waitFor(() => expect(lastSet).toEqual({ group: "git", enabled: true }));
  });

  it("generates the default client's snippet from the resolved setup info", async () => {
    mockIPC((cmd) => {
      if (cmd === "mcp_tool_groups") return DEFAULT_MCP_TOOL_GROUPS;
      if (cmd === "mcp_setup_info") return setupInfo;
      return undefined;
    });

    render(<IntegrationsPanel />);

    // The first client (Claude Code) renders once the resolved helper path arrives; the
    // default data dir emits no env entry. The HTTP API stays read-only beside it.
    await waitFor(() =>
      expect(screen.getByText(/"command": "\/usr\/bin\/soloist-mcp"/)).toBeTruthy(),
    );
    expect(screen.queryByText(/SOLOIST_APP_DATA_DIR/)).toBeNull();
    expect(screen.getByText(/\.mcp\.json \(project root\)/)).toBeTruthy();
    expect(screen.getByText("http://127.0.0.1:24678")).toBeTruthy();
    expect(screen.getByText(`${HTTP_API_ENDPOINTS.length} endpoints`)).toBeTruthy();
  });

  it("copies the generated snippet to the clipboard", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    mockIPC((cmd) => {
      if (cmd === "mcp_tool_groups") return DEFAULT_MCP_TOOL_GROUPS;
      if (cmd === "mcp_setup_info") return setupInfo;
      return undefined;
    });

    render(<IntegrationsPanel />);
    await waitFor(() =>
      expect(screen.getByText(/"command": "\/usr\/bin\/soloist-mcp"/)).toBeTruthy(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));

    await waitFor(() => expect(writeText).toHaveBeenCalledOnce());
    expect(writeText.mock.calls[0][0]).toContain('"command": "/usr/bin/soloist-mcp"');
  });
});
