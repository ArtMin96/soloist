// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { AgentsPanel } from "@/components/settings/AgentsPanel";
import type { AgentTool, DetectedTool } from "@/domain";

const tool = (name: string, command: string): AgentTool => ({
  name,
  command,
  default_args: [],
  kind: "Generic",
  prompt_mode: "Stdin",
});

function mockAgents(opts: { detected: DetectedTool[]; onDetect?: () => void }) {
  mockIPC((cmd) => {
    if (cmd === "agent_list") return opts.detected.map((d) => d.tool);
    if (cmd === "agent_detect") {
      opts.onDetect?.();
      return opts.detected;
    }
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
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.getByText("Installed")).toBeTruthy();
    expect(screen.getByText("Not found")).toBeTruthy();
  });

  it("re-probes the PATH when Detect is clicked", async () => {
    let detects = 0;
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), installed: true }],
      onDetect: () => {
        detects += 1;
      },
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(detects).toBe(1));
    fireEvent.click(screen.getByRole("button", { name: "Detect" }));
    await waitFor(() => expect(detects).toBe(2));
  });

  it("offers no auto-summarization opt-in (the feature is not built)", async () => {
    mockAgents({ detected: [{ tool: tool("Claude", "claude"), installed: true }] });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.queryByText("Auto-summarization")).toBeNull();
    expect(screen.queryByLabelText("Summarizer tool")).toBeNull();
    expect(screen.queryByLabelText("Summarizer model")).toBeNull();
  });
});
