// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { IntegrationsPanel } from "@/components/settings/IntegrationsPanel";
import type { McpFeatureGroup, McpToolGroups } from "@/domain";

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

  it("shows the stdio MCP setup and the read-only HTTP API surface", () => {
    mockIPC((cmd) =>
      cmd === "mcp_tool_groups"
        ? { scratchpads: true, todos: true, timers: true, key_value: false }
        : undefined,
    );

    render(<IntegrationsPanel />);

    // The MCP transport is stdio (no port); the HTTP API is loopback with a derived endpoint count.
    expect(screen.getByText(/"command": "soloist-mcp"/)).toBeTruthy();
    expect(screen.getByText("http://127.0.0.1:24678")).toBeTruthy();
    expect(screen.getByText("16 endpoints")).toBeTruthy();
  });
});
