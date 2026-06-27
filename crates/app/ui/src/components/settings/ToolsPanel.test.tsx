// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ToolsPanel } from "@/components/settings/ToolsPanel";
import type { ToolDefaults } from "@/domain";

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Settings — Tools", () => {
  it("loads the stored tool defaults from the facade on mount", async () => {
    const stored: ToolDefaults = { default_editor: "zed", default_terminal: "kitty" };
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      if (cmd === "tool_defaults") return stored;
      return undefined;
    });

    render(<ToolsPanel />);

    await waitFor(() => expect(calls).toContain("tool_defaults"));
    expect(screen.getByText("Default editor")).toBeTruthy();
    expect(screen.getByText("Default terminal")).toBeTruthy();
  });
});
