// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AgentsPanel } from "@/components/settings/AgentsPanel";
import type { AgentSettings, AgentTool, DetectedTool } from "@/domain";

const tool = (name: string, command: string): AgentTool => ({
  name,
  command,
  default_args: [],
  kind: "Generic",
  prompt_mode: "Stdin",
});

function mockAgents(opts: {
  detected: DetectedTool[];
  settings: AgentSettings;
  onDetect?: () => void;
}) {
  mockIPC((cmd) => {
    if (cmd === "agent_list") return opts.detected.map((d) => d.tool);
    if (cmd === "agent_detect") {
      opts.onDetect?.();
      return opts.detected;
    }
    if (cmd === "agent_settings") return opts.settings;
    return undefined;
  });
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Settings — Agents", () => {
  it("lists detected agent tools with installed status", async () => {
    mockAgents({
      detected: [
        { tool: tool("Claude", "claude"), installed: true },
        { tool: tool("Codex", "codex"), installed: false },
      ],
      settings: { summarizer_tool: null, summarizer_model: null },
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.getByText("Installed")).toBeTruthy();
    expect(screen.getByText("Not found")).toBeTruthy();
  });

  it("enables the model field only when a summarizer tool is chosen", async () => {
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), installed: true }],
      settings: { summarizer_tool: "Claude", summarizer_model: "haiku" },
    });

    render(<AgentsPanel />);

    const model = (await screen.findByLabelText("Summarizer model")) as HTMLInputElement;
    await waitFor(() => expect(model.value).toBe("haiku"));
    expect(model.disabled).toBe(false);
  });

  it("re-probes the PATH when Detect is clicked", async () => {
    let detects = 0;
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), installed: true }],
      settings: { summarizer_tool: null, summarizer_model: null },
      onDetect: () => {
        detects += 1;
      },
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(detects).toBe(1));
    fireEvent.click(screen.getByRole("button", { name: "Detect" }));
    await waitFor(() => expect(detects).toBe(2));
  });
});
