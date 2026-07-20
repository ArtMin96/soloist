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

// Records which detection command each read routed to: the cached sweep (`agent_detect`) or the
// re-probing one (`agent_redetect`). The distinction is the whole point of the Detect button.
function mockAgents(opts: { detected: DetectedTool[] }) {
  const calls: string[] = [];
  mockIPC((cmd) => {
    if (cmd === "agent_list") return opts.detected.map((d) => d.tool);
    if (cmd === "agent_detect" || cmd === "agent_redetect") {
      calls.push(cmd);
      return opts.detected;
    }
    return undefined;
  });
  return calls;
}

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("Settings — Agents", () => {
  it("lists detected agent tools with their detection status", async () => {
    mockAgents({
      detected: [
        { tool: tool("Claude", "claude"), detection: "Installed" },
        { tool: tool("Codex", "codex"), detection: "Missing" },
      ],
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.getByText("installed")).toBeTruthy();
    expect(screen.getByText("not found")).toBeTruthy();
  });

  it("reports a tool the probe could not check as unchecked, not as absent", async () => {
    // A probe that reached no answer must not render as "not found" — that is exactly how a
    // failing probe disguised itself as a machine with no agent CLIs installed.
    mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Unknown" }],
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.getByText("not checked")).toBeTruthy();
    expect(screen.queryByText("not found")).toBeNull();
  });

  it("re-probes rather than re-reading the cached sweep when Detect is clicked", async () => {
    // Reading the cached sweep again is what made this button appear dead: the core serves the
    // same answer for the whole TTL, so an explicit re-check has to bypass it to mean anything.
    const calls = mockAgents({
      detected: [{ tool: tool("Claude", "claude"), detection: "Installed" }],
    });

    render(<AgentsPanel />);

    await waitFor(() => expect(calls).toEqual(["agent_detect"]));
    fireEvent.click(screen.getByRole("button", { name: "Detect" }));
    await waitFor(() => expect(calls).toEqual(["agent_detect", "agent_redetect"]));
  });

  it("offers no auto-summarization opt-in (the feature is not built)", async () => {
    mockAgents({ detected: [{ tool: tool("Claude", "claude"), detection: "Installed" }] });

    render(<AgentsPanel />);

    await waitFor(() => expect(screen.getByText("Claude")).toBeTruthy());
    expect(screen.queryByText("Auto-summarization")).toBeNull();
    expect(screen.queryByLabelText("Summarizer tool")).toBeNull();
    expect(screen.queryByLabelText("Summarizer model")).toBeNull();
  });
});
