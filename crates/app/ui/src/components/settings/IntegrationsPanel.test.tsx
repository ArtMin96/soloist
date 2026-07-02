// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { IntegrationsPanel } from "@/components/settings/IntegrationsPanel";
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
    const groups: McpToolGroups = {
      scratchpads: true,
      todos: true,
      timers: true,
      key_value: false,
    };
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

  it("generates the default client's snippet from the resolved setup info", async () => {
    mockIPC((cmd) => {
      if (cmd === "mcp_tool_groups")
        return { scratchpads: true, todos: true, timers: true, key_value: false };
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
    expect(screen.getByText("19 endpoints")).toBeTruthy();
  });

  it("copies the generated snippet to the clipboard", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    mockIPC((cmd) => {
      if (cmd === "mcp_tool_groups")
        return { scratchpads: true, todos: true, timers: true, key_value: false };
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
