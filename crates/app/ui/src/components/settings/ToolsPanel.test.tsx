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
  it("loads the stored tool defaults and binds them into the selects", async () => {
    const stored: ToolDefaults = { default_editor: "zed", default_terminal: "kitty" };
    const calls: string[] = [];
    mockIPC((cmd) => {
      calls.push(cmd);
      if (cmd === "tool_defaults") return stored;
      return undefined;
    });

    render(<ToolsPanel />);

    await waitFor(() => expect(calls).toContain("tool_defaults"));

    // The stored launch names bind into the controls (their labels, not the raw names): "zed" →
    // "Zed", "kitty" → "kitty". A panel that dropped the loaded value would still render the labels
    // but show "System default" here.
    const editor = screen.getByRole("combobox", { name: "Default editor" });
    const terminal = screen.getByRole("combobox", { name: "Default terminal" });
    await waitFor(() => expect(editor.textContent).toContain("Zed"));
    expect(terminal.textContent).toContain("kitty");
    expect(editor.textContent).not.toContain("System default");
  });
});
